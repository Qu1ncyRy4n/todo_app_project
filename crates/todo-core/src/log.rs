//! Append-only JSONL event log (DI-0007, docs/protocols/todo-ledger-log-v1).

use crate::{GRID_CBOR_CODEC, RAW_CODEC, grid_cbor_cid, raw_cid};
use cid::Cid;
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogLine {
    pub cid: Cid,
    pub bytes: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
#[error("event log line: {0}")]
pub struct LogLineError(String);

impl LogLine {
    pub fn encode(cid: &Cid, bytes: &[u8]) -> Result<String, LogLineError> {
        if event_cid(bytes, cid.codec()) != Some(*cid) {
            return Err(LogLineError(format!("bytes do not hash to cid {cid}")));
        }
        Ok(format!(
            "{{\"cid\":\"{cid}\",\"bytes\":\"{}\"}}",
            hex(bytes)
        ))
    }

    pub fn parse(line: &str) -> Result<Self, LogLineError> {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|error| LogLineError(error.to_string()))?;
        let cid_text = field(&value, "cid")?;
        let bytes_text = field(&value, "bytes")?;
        let cid: Cid = cid_text
            .parse()
            .map_err(|error| LogLineError(format!("invalid cid {cid_text:?}: {error}")))?;
        let bytes = unhex(bytes_text)?;
        if event_cid(&bytes, cid.codec()) != Some(cid) {
            return Err(LogLineError(format!("bytes do not hash to cid {cid_text}")));
        }
        Ok(Self { cid, bytes })
    }
}

fn event_cid(bytes: &[u8], codec: u64) -> Option<Cid> {
    match codec {
        GRID_CBOR_CODEC => Some(grid_cbor_cid(bytes)),
        RAW_CODEC => Some(raw_cid(bytes)),
        _ => None,
    }
}

fn field<'a>(value: &'a Value, name: &'a str) -> Result<&'a str, LogLineError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| LogLineError(format!("missing string field {name:?}")))
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .concat()
}

fn unhex(text: &str) -> Result<Vec<u8>, LogLineError> {
    if !text.len().is_multiple_of(2) {
        return Err(LogLineError("hex text has an odd length".to_owned()));
    }
    (0..text.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&text[index..index + 2], 16)
                .map_err(|error| LogLineError(format!("invalid hex: {error}")))
        })
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "test fixtures use direct setup and assertion values"
)]
mod tests {
    use super::LogLine;
    use crate::{EventBuilder, EventKind, ItemId};

    #[test]
    fn log_lines_round_trip_and_verify_cids() {
        let item = ItemId::parse("0198f58a-1c00-7000-8000-000000000001").unwrap();
        let bytes = EventBuilder::for_item(EventKind::ItemCreated, item)
            .data("title", "Sync me")
            .build()
            .unwrap()
            .bytes;
        let cid = crate::grid_cbor_cid(&bytes);
        let parsed = LogLine::parse(&LogLine::encode(&cid, &bytes).unwrap()).unwrap();
        assert_eq!(parsed.bytes, bytes);
        assert_eq!(
            LogLine::parse(
                "{\"cid\":\"bafkreieeg4aciqnmr2eze5zq5yxmcweoi5zropjjdir7vultwijcsluumq\",\"bytes\":\"ff00\"}"
            )
            .unwrap_err()
            .to_string(),
            "event log line: bytes do not hash to cid bafkreieeg4aciqnmr2eze5zq5yxmcweoi5zropjjdir7vultwijcsluumq"
        );
    }
}
