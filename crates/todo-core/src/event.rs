use crate::{
    GRID_TAG, GridEnvelopeBuilder, GridError, ItemId, NamespaceId, grid::CID_TAG, grid_cbor_cid,
    raw_cid,
};
use chrono::{DateTime, Utc};
use cid::Cid;
use minicbor::{Decoder, Encoder};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};
use thiserror::Error;
use uuid::Uuid;

#[cfg(test)]
const PROTOCOL_V1_BYTES: &[u8] = include_bytes!("../../../docs/protocols/todo-ledger-event-v1.md");
#[cfg(test)]
const PROTOCOL_V2_BYTES: &[u8] = include_bytes!("../../../docs/protocols/todo-ledger-event-v2.md");
#[cfg(test)]
const PROTOCOL_V3_BYTES: &[u8] = include_bytes!("../../../docs/protocols/todo-ledger-event-v3.md");
const PROTOCOL_V4_BYTES: &[u8] = include_bytes!("../../../docs/protocols/todo-ledger-event-v4.md");
const PROTOCOL_V5_BYTES: &[u8] = include_bytes!("../../../docs/protocols/todo-ledger-event-v5.md");
const PROTOCOL_V6_BYTES: &[u8] = include_bytes!("../../../docs/protocols/todo-ledger-event-v6.md");
const PROTOCOL_V7_BYTES: &[u8] = include_bytes!("../../../docs/protocols/todo-ledger-event-v7.md");
const PROTOCOL_V8_BYTES: &[u8] = include_bytes!("../../../docs/protocols/todo-ledger-event-v8.md");
const PROTOCOL_V9_BYTES: &[u8] = include_bytes!("../../../docs/protocols/todo-ledger-event-v9.md");

pub fn event_protocol_v4_cid() -> Cid {
    raw_cid(PROTOCOL_V4_BYTES)
}

pub fn event_protocol_v5_cid() -> Cid {
    raw_cid(PROTOCOL_V5_BYTES)
}

pub fn event_protocol_v6_cid() -> Cid {
    raw_cid(PROTOCOL_V6_BYTES)
}

pub fn event_protocol_v7_cid() -> Cid {
    raw_cid(PROTOCOL_V7_BYTES)
}
pub fn event_protocol_v8_cid() -> Cid {
    raw_cid(PROTOCOL_V8_BYTES)
}
pub fn event_protocol_v9_cid() -> Cid {
    raw_cid(PROTOCOL_V9_BYTES)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EventKind {
    ItemCreated,
    ItemStateChanged,
    ItemNoteAdded,
    ItemCitationAdded,
    ItemParentSet,
    ItemTitleChanged,
    ItemParentCleared,
    ItemProjectChanged,
    ItemPriorityChanged,
    ItemDueDateChanged,
    ItemDeferDateChanged,
    ItemDependencyAdded,
    ItemDependencyRemoved,
    ItemBlockAdded,
    ItemBlockRemoved,
    FocusSequenceCreated,
    FocusSequenceEntryAdded,
    FocusSequenceEntryRemoved,
    ItemSessionStarted,
    ItemSessionFinished,
    ItemPatched,
    NamespaceCreated,
    NamespaceRenamed,
    NamespaceAssigned,
    NamespaceRemoved,
}

impl fmt::Display for EventKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ItemCreated => "item.created",
            Self::ItemStateChanged => "item.state-changed",
            Self::ItemNoteAdded => "item.note-added",
            Self::ItemCitationAdded => "item.citation-added",
            Self::ItemParentSet => "item.parent-set",
            Self::ItemTitleChanged => "item.title-changed",
            Self::ItemParentCleared => "item.parent-cleared",
            Self::ItemProjectChanged => "item.project-changed",
            Self::ItemPriorityChanged => "item.priority-changed",
            Self::ItemDueDateChanged => "item.due-date-changed",
            Self::ItemDeferDateChanged => "item.defer-date-changed",
            Self::ItemDependencyAdded => "item.dependency-added",
            Self::ItemDependencyRemoved => "item.dependency-removed",
            Self::ItemBlockAdded => "item.block-added",
            Self::ItemBlockRemoved => "item.block-removed",
            Self::FocusSequenceCreated => "focus-sequence.created",
            Self::FocusSequenceEntryAdded => "focus-sequence.entry-added",
            Self::FocusSequenceEntryRemoved => "focus-sequence.entry-removed",
            Self::ItemSessionStarted => "item.session-started",
            Self::ItemSessionFinished => "item.session-finished",
            Self::ItemPatched => "item.patched",
            Self::NamespaceCreated => "namespace.created",
            Self::NamespaceRenamed => "namespace.renamed",
            Self::NamespaceAssigned => "namespace.assigned",
            Self::NamespaceRemoved => "namespace.removed",
        })
    }
}

impl std::str::FromStr for EventKind {
    type Err = EventDecodeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Ok(match text {
            "item.created" => Self::ItemCreated,
            "item.state-changed" => Self::ItemStateChanged,
            "item.note-added" => Self::ItemNoteAdded,
            "item.citation-added" => Self::ItemCitationAdded,
            "item.parent-set" => Self::ItemParentSet,
            "item.title-changed" => Self::ItemTitleChanged,
            "item.parent-cleared" => Self::ItemParentCleared,
            "item.project-changed" => Self::ItemProjectChanged,
            "item.priority-changed" => Self::ItemPriorityChanged,
            "item.due-date-changed" => Self::ItemDueDateChanged,
            "item.defer-date-changed" => Self::ItemDeferDateChanged,
            "item.dependency-added" => Self::ItemDependencyAdded,
            "item.dependency-removed" => Self::ItemDependencyRemoved,
            "item.block-added" => Self::ItemBlockAdded,
            "item.block-removed" => Self::ItemBlockRemoved,
            "focus-sequence.created" => Self::FocusSequenceCreated,
            "focus-sequence.entry-added" => Self::FocusSequenceEntryAdded,
            "focus-sequence.entry-removed" => Self::FocusSequenceEntryRemoved,
            "item.session-started" => Self::ItemSessionStarted,
            "item.session-finished" => Self::ItemSessionFinished,
            "item.patched" => Self::ItemPatched,
            "namespace.created" => Self::NamespaceCreated,
            "namespace.renamed" => Self::NamespaceRenamed,
            "namespace.assigned" => Self::NamespaceAssigned,
            "namespace.removed" => Self::NamespaceRemoved,
            other => {
                return Err(EventDecodeError::Invalid(format!(
                    "unknown event kind {other:?}"
                )));
            }
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventLink {
    pub relation: String,
    pub target: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventPayload {
    pub event_id: Uuid,
    pub kind: EventKind,
    pub actor_id: String,
    pub subject_id: String,
    pub project_id: Option<NamespaceId>,
    pub recorded_at: DateTime<Utc>,
    pub data: BTreeMap<String, String>,
    pub links: Vec<EventLink>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub cid: Cid,
    pub parent: Option<Cid>,
    pub payload: EventPayload,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct EventBuilder {
    payload: EventPayload,
    parent: Option<Cid>,
}

impl EventBuilder {
    pub fn new(kind: EventKind, subject_id: impl Into<String>) -> Self {
        Self {
            payload: EventPayload {
                event_id: Uuid::now_v7(),
                kind,
                actor_id: "local".to_owned(),
                subject_id: subject_id.into(),
                project_id: None,
                recorded_at: Utc::now(),
                data: BTreeMap::new(),
                links: Vec::new(),
            },
            parent: None,
        }
    }

    pub fn for_item(kind: EventKind, item_id: ItemId) -> Self {
        Self::new(kind, item_id.to_string())
    }

    pub fn parent(mut self, parent: Option<Cid>) -> Self {
        self.parent = parent;
        self
    }

    pub fn project(mut self, project_id: Option<NamespaceId>) -> Self {
        self.payload.project_id = project_id;
        self
    }

    pub fn at(mut self, recorded_at: DateTime<Utc>) -> Self {
        self.payload.recorded_at = recorded_at;
        self
    }

    pub fn actor(mut self, actor_id: impl Into<String>) -> Self {
        self.payload.actor_id = actor_id.into();
        self
    }

    pub fn data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.payload.data.insert(key.into(), value.into());
        self
    }

    pub fn link(mut self, relation: impl Into<String>, target: impl Into<String>) -> Self {
        self.payload.links.push(EventLink {
            relation: relation.into(),
            target: target.into(),
        });
        self
    }

    pub fn build(self) -> Result<Event, EventEncodeError> {
        // All new writes use the v9 Grid-CBOR protocol. Historical rows are
        // never rebuilt or relabeled. Source: DI-0008.
        let protocol_cid = event_protocol_v9_cid();
        let bytes = encode_envelope(&protocol_cid, self.parent.as_ref(), &self.payload)?;
        let cid = grid_cbor_cid(&bytes);
        Ok(Event {
            cid,
            parent: self.parent,
            payload: self.payload,
            bytes,
        })
    }
}

#[derive(Debug, Error)]
pub enum EventEncodeError {
    #[error("encode event slot: {0}")]
    Encode(#[from] minicbor::encode::Error<std::convert::Infallible>),
    #[error("build Grid envelope: {0}")]
    Grid(#[from] GridError),
    #[error("event has too many map or array entries to encode")]
    TooManyEntries,
}

#[derive(Debug, Error)]
pub enum EventDecodeError {
    #[error("decode event: {0}")]
    Cbor(#[from] minicbor::decode::Error),
    #[error("decode event: {0}")]
    Invalid(String),
    #[error("decode event timestamp: {0}")]
    Timestamp(#[from] chrono::ParseError),
}

pub fn decode_envelope(bytes: &[u8]) -> Result<Event, EventDecodeError> {
    let cid = grid_cbor_cid(bytes);
    let mut decoder = Decoder::new(bytes);
    let grid = decoder.tag()?;
    if grid.as_u64() != GRID_TAG {
        return Err(EventDecodeError::Invalid(
            "envelope is not Grid-framed".to_owned(),
        ));
    }
    if decoder.array()? != Some(4) {
        return Err(EventDecodeError::Invalid(
            "envelope is not a 4-element Grid array".to_owned(),
        ));
    }
    decode_cid(&mut decoder)?;
    let parent = match decoder.array()? {
        Some(0) => None,
        Some(1) => Some(decode_cid(&mut decoder)?),
        _ => return Err(EventDecodeError::Invalid("invalid parent count".to_owned())),
    };
    if decoder.map()? != Some(10) {
        return Err(EventDecodeError::Invalid(
            "envelope payload is not a 10-entry map".to_owned(),
        ));
    }
    expect_field(&mut decoder, "schema_version")?;
    let _schema_version = decoder.u8()?;
    expect_field(&mut decoder, "action")?;
    if decoder.str()? != "record" {
        return Err(EventDecodeError::Invalid(
            "envelope action is not record".to_owned(),
        ));
    }
    expect_field(&mut decoder, "kind")?;
    let kind: EventKind = decoder.str()?.parse()?;
    expect_field(&mut decoder, "event_id")?;
    let event_id = Uuid::parse_str(decoder.str()?)
        .map_err(|error| EventDecodeError::Invalid(format!("invalid event id: {error}")))?;
    expect_field(&mut decoder, "actor_id")?;
    let actor_id = decoder.str()?.to_owned();
    expect_field(&mut decoder, "subject_id")?;
    let subject_id = decoder.str()?.to_owned();
    expect_field(&mut decoder, "project_id")?;
    let project_id = match decoder.datatype()? {
        minicbor::data::Type::Null => {
            decoder.skip()?;
            None
        }
        _ => {
            let text = decoder.str()?;
            let id = NamespaceId::parse(text).map_err(|error| {
                EventDecodeError::Invalid(format!("invalid project id: {error}"))
            })?;
            Some(id)
        }
    };
    expect_field(&mut decoder, "recorded_at")?;
    let recorded_at = DateTime::parse_from_rfc3339(decoder.str()?)?.to_utc();
    expect_field(&mut decoder, "data")?;
    let entries = decoder.map()?;
    let mut data = BTreeMap::new();
    for _ in 0..entries.unwrap_or(0) {
        let key = decoder.str()?.to_owned();
        let value = decoder.str()?.to_owned();
        data.insert(key, value);
    }
    expect_field(&mut decoder, "links")?;
    let link_count = decoder.array()?;
    let mut links = Vec::new();
    for _ in 0..link_count.unwrap_or(0) {
        if decoder.map()? != Some(2) {
            return Err(EventDecodeError::Invalid(
                "link is not a 2-entry map".to_owned(),
            ));
        }
        expect_field(&mut decoder, "relation")?;
        let relation = decoder.str()?.to_owned();
        expect_field(&mut decoder, "target")?;
        let target = decoder.str()?.to_owned();
        links.push(EventLink { relation, target });
    }
    if decoder.datatype()? != minicbor::data::Type::Null {
        return Err(EventDecodeError::Invalid(
            "expected a null proof slot".to_owned(),
        ));
    }
    decoder.skip()?;
    if decoder.datatype().is_ok() {
        return Err(EventDecodeError::Invalid(
            "trailing bytes after envelope".to_owned(),
        ));
    }
    Ok(Event {
        cid,
        parent,
        payload: EventPayload {
            event_id,
            kind,
            actor_id,
            subject_id,
            project_id,
            recorded_at,
            data,
            links,
        },
        bytes: bytes.to_vec(),
    })
}

fn expect_field(decoder: &mut Decoder<'_>, name: &str) -> Result<(), EventDecodeError> {
    if decoder.str()? != name {
        return Err(EventDecodeError::Invalid(format!(
            "expected field {name:?}"
        )));
    }
    Ok(())
}

fn decode_cid(decoder: &mut Decoder<'_>) -> Result<Cid, EventDecodeError> {
    let tag = decoder.tag()?;
    if tag.as_u64() != CID_TAG {
        return Err(EventDecodeError::Invalid("expected CID tag 42".to_owned()));
    }
    let content = decoder.bytes()?;
    let Some((&marker, body)) = content.split_first() else {
        return Err(EventDecodeError::Invalid("empty CID content".to_owned()));
    };
    if marker != 0 {
        return Err(EventDecodeError::Invalid(
            "unsupported CID content marker".to_owned(),
        ));
    }
    Cid::try_from(body).map_err(|error| EventDecodeError::Invalid(format!("invalid CID: {error}")))
}

fn encode_envelope(
    protocol_cid: &Cid,
    parent: Option<&Cid>,
    payload: &EventPayload,
) -> Result<Vec<u8>, EventEncodeError> {
    let mut parents = Encoder::new(Vec::new());
    parents.array(u64::from(parent.is_some()))?;
    if let Some(parent) = parent {
        crate::grid::encode_cid(&mut parents, parent)?;
    }

    let mut body = Encoder::new(Vec::new());
    body.map(10)?;
    body.str("schema_version")?.u8(1)?;
    body.str("action")?.str("record")?;
    body.str("kind")?.str(&payload.kind.to_string())?;
    body.str("event_id")?.str(&payload.event_id.to_string())?;
    body.str("actor_id")?.str(&payload.actor_id)?;
    body.str("subject_id")?.str(&payload.subject_id)?;
    body.str("project_id")?;
    match payload.project_id {
        Some(project_id) => body.str(&project_id.to_string())?,
        None => body.null()?,
    };
    body.str("recorded_at")?
        .str(&payload.recorded_at.to_rfc3339())?;
    let data_len =
        u64::try_from(payload.data.len()).map_err(|_| EventEncodeError::TooManyEntries)?;
    body.str("data")?.map(data_len)?;
    for (key, value) in &payload.data {
        body.str(key)?.str(value)?;
    }
    let link_len =
        u64::try_from(payload.links.len()).map_err(|_| EventEncodeError::TooManyEntries)?;
    body.str("links")?.array(link_len)?;
    for link in &payload.links {
        body.map(2)?
            .str("relation")?
            .str(&link.relation)?
            .str("target")?
            .str(&link.target)?;
    }

    let mut builder = GridEnvelopeBuilder::new(protocol_cid.to_owned());
    builder.put_slot(1, &parents.into_writer())?;
    builder.put_slot(2, &body.into_writer())?;
    builder.put_slot(3, &[0xf6])?;
    Ok(builder.build()?.bytes().to_vec())
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "test fixtures use direct setup and assertion values"
)]
mod tests {
    use super::{
        EventBuilder, EventKind, PROTOCOL_V1_BYTES, PROTOCOL_V2_BYTES, PROTOCOL_V3_BYTES,
        PROTOCOL_V4_BYTES, PROTOCOL_V5_BYTES, PROTOCOL_V9_BYTES,
    };
    use crate::{GRID_CBOR_CODEC, GRID_TAG, GridEnvelope, ItemId, raw_cid};
    use chrono::DateTime;
    use minicbor::{Decoder, data::Type};

    #[test]
    fn event_encoding_is_deterministic_and_grid_framed() {
        let time = DateTime::parse_from_rfc3339("2026-08-27T12:00:00Z")
            .unwrap()
            .to_utc();
        let item = ItemId::parse("0198f58a-1c00-7000-8000-000000000001").unwrap();
        let build = || {
            EventBuilder::for_item(EventKind::ItemCreated, item)
                .at(time)
                .data("title", "Review onboarding")
                .build()
                .unwrap()
        };
        let first = build();
        let second = build();
        assert_ne!(
            first.cid, second.cid,
            "event IDs distinguish separate records"
        );

        let mut decoder = Decoder::new(&first.bytes);
        assert_eq!(decoder.tag().unwrap().as_u64(), GRID_TAG);
        assert_eq!(decoder.array().unwrap(), Some(4));
        assert_eq!(decoder.datatype().unwrap(), Type::Tag);
        assert_eq!(first.cid.codec(), GRID_CBOR_CODEC);

        let parsed = GridEnvelope::parse(&first.bytes).unwrap();
        assert_eq!(parsed.slot_count(), 3);
        assert_eq!(parsed.protocol_cid(), &raw_cid(PROTOCOL_V9_BYTES));
        assert_eq!(parsed.cid(), first.cid);
    }

    #[test]
    fn a_parent_changes_the_exact_event() {
        let first = EventBuilder::new(EventKind::ItemCreated, "one")
            .build()
            .unwrap();
        let second = EventBuilder::new(EventKind::ItemNoteAdded, "one")
            .parent(Some(first.cid))
            .data("note", "hello")
            .build()
            .unwrap();
        assert_eq!(second.parent, Some(first.cid));
        assert_ne!(first.cid, second.cid);
    }

    #[test]
    fn v5_event_can_extend_a_legacy_raw_codec_head() {
        let legacy_head = raw_cid(b"legacy event bytes");
        let event = EventBuilder::new(EventKind::ItemNoteAdded, "one")
            .parent(Some(legacy_head.to_owned()))
            .data("note", "continued after migration")
            .build()
            .unwrap();

        assert_eq!(event.parent.as_ref(), Some(&legacy_head));
        assert_eq!(event.cid.codec(), GRID_CBOR_CODEC);
        GridEnvelope::parse(&event.bytes).unwrap();
    }

    #[test]
    fn protocol_versions_have_distinct_content_identities() {
        assert_eq!(
            raw_cid(PROTOCOL_V1_BYTES).to_string(),
            "bafkreieeg4aciqnmr2eze5zq5yxmcweoi5zropjjdir7vultwijcsluumq"
        );
        assert_eq!(
            raw_cid(PROTOCOL_V2_BYTES).to_string(),
            "bafkreib343dtizusqbavpevtkbfxmqdqshfl2tgd2uq7iqg2boi2pldlha"
        );
        assert_ne!(raw_cid(PROTOCOL_V2_BYTES), raw_cid(PROTOCOL_V3_BYTES));
        assert_ne!(raw_cid(PROTOCOL_V3_BYTES), raw_cid(PROTOCOL_V4_BYTES));
        assert_ne!(raw_cid(PROTOCOL_V4_BYTES), raw_cid(PROTOCOL_V5_BYTES));
        assert_eq!(
            raw_cid(PROTOCOL_V4_BYTES).to_string(),
            "bafkreie4yxx5msmhvfcspqoh55v5mwt3htr7sc34yx54igg5usx2yiylie"
        );
    }
}
