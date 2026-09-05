use cid::{Cid, Version};
use minicbor::{Encoder, data::Tag};
use multihash::Multihash;
use sha2::{Digest, Sha256};
use std::{io::Cursor, ops::Range};
use thiserror::Error;

pub const GRID_TAG: u64 = 1_735_551_332;
pub const CID_TAG: u64 = 42;
pub const GRID_CBOR_CODEC: u64 = 0x1027;
pub const RAW_CODEC: u64 = 0x55;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridEnvelope {
    bytes: Vec<u8>,
    protocol_cid: Cid,
    slots: Vec<Range<usize>>,
}

impl GridEnvelope {
    pub fn parse(bytes: &[u8]) -> Result<Self, GridError> {
        let bytes = bytes.to_vec();
        let mut scanner = Scanner::new(&bytes);
        scanner.scan_complete().map_err(GridError::invalid_cbor)?;
        let framing = scanner
            .scan_envelope()
            .map_err(GridError::invalid_envelope)?;
        let protocol_cid = scanner.validate_protocol_cid(&framing.protocol)?;
        scanner.validate_visible_cids()?;
        Ok(Self {
            bytes,
            protocol_cid,
            slots: framing.slots,
        })
    }

    pub fn protocol_cid(&self) -> &Cid {
        &self.protocol_cid
    }

    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    pub fn slot(&self, number: usize) -> Result<&[u8], GridError> {
        if number == 0 {
            return Err(GridError::InvalidSlot(number));
        }
        let range = self
            .slots
            .get(number - 1)
            .ok_or(GridError::InvalidSlot(number))?;
        Ok(&self.bytes[range.clone()])
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn cid(&self) -> Cid {
        grid_cbor_cid(&self.bytes)
    }
}

#[derive(Clone, Debug)]
pub struct GridEnvelopeBuilder {
    protocol_cid: Cid,
    slots: Vec<Option<Vec<u8>>>,
    closed: bool,
}

impl GridEnvelopeBuilder {
    pub fn new(protocol_cid: Cid) -> Self {
        Self {
            protocol_cid,
            slots: Vec::new(),
            closed: false,
        }
    }

    pub fn put_slot(&mut self, number: usize, encoded_item: &[u8]) -> Result<(), GridError> {
        if self.closed {
            return Err(GridError::BuilderClosed);
        }
        if number == 0 {
            return Err(GridError::InvalidSlot(number));
        }
        if self.slots.len() < number {
            self.slots.resize_with(number, || None);
        }
        self.slots[number - 1] = Some(encoded_item.to_vec());
        Ok(())
    }

    pub fn build(&mut self) -> Result<GridEnvelope, GridError> {
        if self.closed {
            return Err(GridError::BuilderClosed);
        }
        if self.protocol_cid.version() != Version::V1 {
            return Err(GridError::InvalidCid(
                "builder pCID must be CIDv1".to_owned(),
            ));
        }
        for (index, slot) in self.slots.iter().enumerate() {
            let slot = slot.as_ref().ok_or(GridError::MissingSlot(index + 1))?;
            validate_item(slot)?;
        }

        let mut encoder = Encoder::new(Vec::new());
        encoder
            .tag(Tag::new(GRID_TAG))
            .and_then(|encoder| encoder.array((self.slots.len() + 1) as u64))
            .map_err(|error| GridError::InvalidCbor(error.to_string()))?;
        encode_cid(&mut encoder, &self.protocol_cid)
            .map_err(|error| GridError::InvalidCbor(error.to_string()))?;
        for slot in &self.slots {
            encoder
                .writer_mut()
                .extend_from_slice(slot.as_deref().expect("slot gaps checked"));
        }
        let envelope = GridEnvelope::parse(&encoder.into_writer())?;
        self.closed = true;
        self.slots.clear();
        Ok(envelope)
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum GridError {
    #[error("invalid CBOR: {0}")]
    InvalidCbor(String),
    #[error("invalid Grid envelope: {0}")]
    InvalidEnvelope(String),
    #[error("invalid CID: {0}")]
    InvalidCid(String),
    #[error("invalid slot {0}")]
    InvalidSlot(usize),
    #[error("missing slot {0}")]
    MissingSlot(usize),
    #[error("builder is closed")]
    BuilderClosed,
}

impl GridError {
    fn invalid_cbor(message: impl ToString) -> Self {
        Self::InvalidCbor(message.to_string())
    }

    fn invalid_envelope(message: impl ToString) -> Self {
        Self::InvalidEnvelope(message.to_string())
    }
}

pub fn raw_cid(bytes: &[u8]) -> Cid {
    cid_for(RAW_CODEC, bytes)
}

pub fn grid_cbor_cid(bytes: &[u8]) -> Cid {
    cid_for(GRID_CBOR_CODEC, bytes)
}

pub(crate) fn encode_cid(
    encoder: &mut Encoder<Vec<u8>>,
    cid: &Cid,
) -> Result<(), minicbor::encode::Error<std::convert::Infallible>> {
    let mut content = Vec::with_capacity(cid.encoded_len() + 1);
    content.push(0);
    content.extend_from_slice(&cid.to_bytes());
    encoder.tag(Tag::new(CID_TAG))?.bytes(&content)?;
    Ok(())
}

fn cid_for(codec: u64, bytes: &[u8]) -> Cid {
    let digest = Sha256::digest(bytes);
    let hash = Multihash::<64>::wrap(0x12, &digest).expect("SHA-256 fits the CID digest");
    Cid::new_v1(codec, hash)
}

fn validate_item(bytes: &[u8]) -> Result<(), GridError> {
    let mut scanner = Scanner::new(bytes);
    scanner.scan_complete().map_err(GridError::invalid_cbor)?;
    scanner.validate_visible_cids()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FrameKind {
    Items,
    ByteChunks,
    TextChunks,
    MapItems,
}

#[derive(Clone, Copy, Debug)]
struct Frame {
    kind: FrameKind,
    remaining: u64,
    indefinite: bool,
    item_count: u64,
}

#[derive(Debug)]
struct Framing {
    protocol: Range<usize>,
    slots: Vec<Range<usize>>,
}

struct Scanner<'a> {
    bytes: &'a [u8],
    visible_cids: Vec<usize>,
}

impl<'a> Scanner<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            visible_cids: Vec::new(),
        }
    }

    fn scan_complete(&mut self) -> Result<(), String> {
        if self.bytes.is_empty() {
            return Err("input is empty".to_owned());
        }
        self.visible_cids.clear();
        let end = self.scan_item(0, true)?;
        if end != self.bytes.len() {
            return Err(format!("trailing bytes begin at offset {end}"));
        }
        Ok(())
    }

    fn scan_item(&mut self, offset: usize, collect_cids: bool) -> Result<usize, String> {
        let mut position = offset;
        let mut frames = vec![Frame {
            kind: FrameKind::Items,
            remaining: 1,
            indefinite: false,
            item_count: 0,
        }];
        while let Some(frame) = frames.last_mut() {
            if !frame.indefinite && frame.remaining == 0 {
                frames.pop();
                continue;
            }
            let Some(&head) = self.bytes.get(position) else {
                return Err(format!("item starting at offset {offset} is truncated"));
            };
            if head == 0xff {
                if !frame.indefinite {
                    return Err(format!("unexpected break at offset {position}"));
                }
                if frame.kind == FrameKind::MapItems && frame.item_count % 2 != 0 {
                    return Err(format!(
                        "indefinite map has an unmatched key before offset {position}"
                    ));
                }
                position += 1;
                frames.pop();
                continue;
            }
            if frame.indefinite {
                frame.item_count += 1;
            } else {
                frame.remaining -= 1;
            }

            let frame_kind = frame.kind;
            let (major, argument, indefinite, next) = self.read_argument(position)?;
            if matches!(frame_kind, FrameKind::ByteChunks | FrameKind::TextChunks) {
                let expected = if frame_kind == FrameKind::ByteChunks {
                    2
                } else {
                    3
                };
                if major != expected || indefinite {
                    return Err(format!(
                        "invalid indefinite-string chunk at offset {position}"
                    ));
                }
                let end = self.read_payload(next, argument)?;
                if major == 3 && std::str::from_utf8(&self.bytes[next..end]).is_err() {
                    return Err(format!("text string at offset {position} is not UTF-8"));
                }
                position = end;
                continue;
            }

            let item_start = position;
            position = next;
            match major {
                0 | 1 => {
                    if indefinite {
                        return Err(format!("integer at offset {item_start} is indefinite"));
                    }
                }
                2 | 3 => {
                    if indefinite {
                        frames.push(Frame {
                            kind: if major == 2 {
                                FrameKind::ByteChunks
                            } else {
                                FrameKind::TextChunks
                            },
                            remaining: 0,
                            indefinite: true,
                            item_count: 0,
                        });
                    } else {
                        let end = self.read_payload(position, argument)?;
                        if major == 3 && std::str::from_utf8(&self.bytes[position..end]).is_err() {
                            return Err(format!("text string at offset {item_start} is not UTF-8"));
                        }
                        position = end;
                    }
                }
                4 => frames.push(Frame {
                    kind: FrameKind::Items,
                    remaining: argument,
                    indefinite,
                    item_count: 0,
                }),
                5 => frames.push(Frame {
                    kind: FrameKind::MapItems,
                    remaining: argument
                        .checked_mul(2)
                        .ok_or_else(|| format!("map at offset {item_start} is too large"))?,
                    indefinite,
                    item_count: 0,
                }),
                6 => {
                    if indefinite {
                        return Err(format!("tag at offset {item_start} is indefinite"));
                    }
                    if collect_cids && argument == CID_TAG {
                        self.visible_cids.push(position);
                    }
                    frames.push(Frame {
                        kind: FrameKind::Items,
                        remaining: 1,
                        indefinite: false,
                        item_count: 0,
                    });
                }
                7 => {
                    if indefinite {
                        return Err(format!("break at offset {item_start} is misplaced"));
                    }
                }
                _ => return Err(format!("unsupported CBOR major type {major}")),
            }
        }
        Ok(position)
    }

    fn scan_envelope(&mut self) -> Result<Framing, String> {
        let (major, tag, indefinite, position) = self.read_argument(0)?;
        if major != 6 || indefinite || tag != GRID_TAG {
            return Err(format!("outer item is not Grid tag {GRID_TAG}"));
        }
        let (major, slot_count, indefinite, mut position) = self.read_argument(position)?;
        if major != 4 {
            return Err("Grid tag content is not an array".to_owned());
        }
        let mut ranges = Vec::new();
        let mut wire_slot = 0_u64;
        loop {
            if indefinite {
                let Some(&head) = self.bytes.get(position) else {
                    return Err("indefinite Grid array is truncated".to_owned());
                };
                if head == 0xff {
                    position += 1;
                    break;
                }
            } else if wire_slot == slot_count {
                break;
            }
            let start = position;
            position = self.scan_item(position, false)?;
            ranges.push(start..position);
            wire_slot += 1;
        }
        if ranges.is_empty() {
            return Err("Grid array has no pCID in slot zero".to_owned());
        }
        if position != self.bytes.len() {
            return Err(format!(
                "Grid tag content ends at offset {position}, not {}",
                self.bytes.len()
            ));
        }
        Ok(Framing {
            protocol: ranges.remove(0),
            slots: ranges,
        })
    }

    fn validate_protocol_cid(&self, range: &Range<usize>) -> Result<Cid, GridError> {
        let (major, tag, indefinite, child) = self
            .read_argument(range.start)
            .map_err(GridError::invalid_envelope)?;
        if major != 6 || indefinite || tag != CID_TAG {
            return Err(GridError::InvalidEnvelope(
                "slot zero is not tag 42".to_owned(),
            ));
        }
        let (cid, end) = self.validate_cid(child)?;
        if end != range.end {
            return Err(GridError::InvalidCid(
                "pCID tag contains more than one item".to_owned(),
            ));
        }
        Ok(cid)
    }

    fn validate_visible_cids(&self) -> Result<(), GridError> {
        for &offset in &self.visible_cids {
            self.validate_cid(offset)?;
        }
        Ok(())
    }

    fn validate_cid(&self, offset: usize) -> Result<(Cid, usize), GridError> {
        let (major, length, indefinite, start) =
            self.read_argument(offset).map_err(GridError::InvalidCid)?;
        if major != 2 || indefinite {
            return Err(GridError::InvalidCid(
                "tag 42 content is not a definite byte string".to_owned(),
            ));
        }
        let end = self
            .read_payload(start, length)
            .map_err(GridError::InvalidCid)?;
        let content = &self.bytes[start..end];
        if content.len() < 2 || content[0] != 0 {
            return Err(GridError::InvalidCid(
                "tag 42 content lacks the leading zero sentinel".to_owned(),
            ));
        }
        let mut cursor = Cursor::new(&content[1..]);
        let cid = Cid::read_bytes(&mut cursor)
            .map_err(|error| GridError::InvalidCid(error.to_string()))?;
        if cursor.position() as usize != content.len() - 1 {
            return Err(GridError::InvalidCid(
                "tag 42 content contains trailing bytes".to_owned(),
            ));
        }
        if cid.version() != Version::V1 {
            return Err(GridError::InvalidCid(
                "tag 42 content is not CIDv1".to_owned(),
            ));
        }
        Ok((cid, end))
    }

    fn read_argument(&self, offset: usize) -> Result<(u8, u64, bool, usize), String> {
        let head = *self
            .bytes
            .get(offset)
            .ok_or_else(|| format!("missing CBOR head at offset {offset}"))?;
        let major = head >> 5;
        let additional = head & 0x1f;
        let next = offset + 1;
        match additional {
            0..=23 => Ok((major, u64::from(additional), false, next)),
            24 => self
                .read_unsigned(next, 1)
                .map(|(value, end)| (major, value, false, end)),
            25 => self
                .read_unsigned(next, 2)
                .map(|(value, end)| (major, value, false, end)),
            26 => self
                .read_unsigned(next, 4)
                .map(|(value, end)| (major, value, false, end)),
            27 => self
                .read_unsigned(next, 8)
                .map(|(value, end)| (major, value, false, end)),
            31 => Ok((major, 0, true, next)),
            _ => Err(format!(
                "reserved additional information {additional} at offset {offset}"
            )),
        }
    }

    fn read_unsigned(&self, offset: usize, length: usize) -> Result<(u64, usize), String> {
        let end = offset
            .checked_add(length)
            .ok_or_else(|| "CBOR argument offset overflow".to_owned())?;
        let source = self
            .bytes
            .get(offset..end)
            .ok_or_else(|| format!("CBOR argument at offset {offset} needs {length} bytes"))?;
        let value = match length {
            1 => u64::from(source[0]),
            2 => u64::from(u16::from_be_bytes([source[0], source[1]])),
            4 => u64::from(u32::from_be_bytes(
                source.try_into().expect("four-byte slice"),
            )),
            8 => u64::from_be_bytes(source.try_into().expect("eight-byte slice")),
            _ => return Err(format!("unsupported CBOR argument length {length}")),
        };
        Ok((value, end))
    }

    fn read_payload(&self, offset: usize, length: u64) -> Result<usize, String> {
        let length = usize::try_from(length)
            .map_err(|_| format!("payload at offset {offset} exceeds platform size"))?;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| format!("payload at offset {offset} overflows"))?;
        if end > self.bytes.len() {
            return Err(format!(
                "payload at offset {offset} declares {length} bytes with only {} available",
                self.bytes.len().saturating_sub(offset)
            ));
        }
        Ok(end)
    }
}

#[cfg(test)]
mod tests {
    use super::{GridEnvelope, GridEnvelopeBuilder, GridError, grid_cbor_cid};

    const VALID_EMPTY: &str = "da6772696481d82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let text = std::str::from_utf8(pair).unwrap();
                u8::from_str_radix(text, 16).unwrap()
            })
            .collect()
    }

    #[test]
    fn private_grid_cbor_vectors_match() {
        let valid = [
            (
                VALID_EMPTY,
                "bagtsaera7xid4ecjsttiv5lzyjxnm7g7mfmwgj7uiewxkrwtqqn3c3hzifzq",
            ),
            (
                "da6772696482d82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855fb3ff8000000000000",
                "bagtsaeral4fy7jnvkltwp6tc5txmb6rswtluialeswntvpizzfl5p473ijma",
            ),
            (
                "da6772696482d82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855a101d903e86472696368",
                "bagtsaeraj4qhm6g5e7pzqvcyexu3p5e6iuxork6ayf2zsifnpwhkbtsppxxa",
            ),
            (
                "da677269649fd82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b8557f626f6bffff",
                "bagtsaeraoo5d2ofnsmizw5p4vluc4hb7q32mapgnqmlnbgenhirpwgqzwcwq",
            ),
            (
                "da677269649801d82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                "bagtsaerach2a3yeme443a6xebupmrmrud44prb73crdqeihh26mrkifbtulq",
            ),
            (
                "da6772696482d82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855d82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                "bagtsaerazluxzhlxzff4xuherroz4qbudz65gpmw7ox6sp5v5jqbusjo3lga",
            ),
            (
                "da6772696482d82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b85546d82a43000155",
                "bagtsaera53dwvv6htysxiluzkvafk7chyyjoit3pnvsxzr3bz7fp3uhhf4yq",
            ),
        ];
        for (encoded, expected_cid) in valid {
            let bytes = hex(encoded);
            let envelope = GridEnvelope::parse(&bytes).unwrap();
            assert_eq!(grid_cbor_cid(&bytes).to_string(), expected_cid);
            assert_eq!(envelope.cid().to_string(), expected_cid);
        }
    }

    #[test]
    fn private_invalid_vectors_are_rejected() {
        let invalid = [
            "da6772696481d82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b85500",
            "da6772696581d82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "da67726964f6",
            "da6772696480",
            "da677269648100",
            "da6772696481d82a5823001220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "da6772696482d82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855d82a43000155",
            "da6772696482d82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b85561ff",
            "da6772696482d82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ];
        for encoded in invalid {
            assert!(
                GridEnvelope::parse(&hex(encoded)).is_err(),
                "accepted {encoded}"
            );
        }
    }

    #[test]
    fn builder_preserves_protocol_slots_and_closes() {
        let parsed = GridEnvelope::parse(&hex(VALID_EMPTY)).unwrap();
        let mut builder = GridEnvelopeBuilder::new(parsed.protocol_cid().to_owned());
        builder.put_slot(2, &[0x82, 0x02, 0x03]).unwrap();
        builder.put_slot(1, &[0x18, 0x01]).unwrap();
        let built = builder.build().unwrap();
        assert_eq!(built.slot(1).unwrap(), [0x18, 0x01]);
        assert_eq!(built.slot(2).unwrap(), [0x82, 0x02, 0x03]);
        assert_eq!(builder.build(), Err(GridError::BuilderClosed));
    }

    #[test]
    fn dag_cbor_slot_bytes_round_trip_unchanged() {
        let bytes = hex(
            "da6772696482d82a58250001551220e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855fb3ff8000000000000",
        );
        let parsed = GridEnvelope::parse(&bytes).unwrap();
        assert_eq!(parsed.slot(1).unwrap(), hex("fb3ff8000000000000"));
    }
}
