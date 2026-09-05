use crate::{GridEnvelopeBuilder, GridError, ItemId, NamespaceId, grid_cbor_cid, raw_cid};
use chrono::{DateTime, Utc};
use cid::Cid;
use minicbor::Encoder;
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
            Self::NamespaceCreated => "namespace.created",
            Self::NamespaceRenamed => "namespace.renamed",
            Self::NamespaceAssigned => "namespace.assigned",
            Self::NamespaceRemoved => "namespace.removed",
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
        // All new writes use the v4 Grid-CBOR codec. Historical v1-v3 rows are
        // never rebuilt or relabeled. Source: DI-0008.
        let protocol_cid = raw_cid(PROTOCOL_V4_BYTES);
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
    body.str("data")?.map(payload.data.len() as u64)?;
    for (key, value) in &payload.data {
        body.str(key)?.str(value)?;
    }
    body.str("links")?.array(payload.links.len() as u64)?;
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
mod tests {
    use super::{
        EventBuilder, EventKind, PROTOCOL_V1_BYTES, PROTOCOL_V2_BYTES, PROTOCOL_V3_BYTES,
        PROTOCOL_V4_BYTES,
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
        assert_eq!(parsed.protocol_cid(), &raw_cid(PROTOCOL_V4_BYTES));
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
    fn v4_event_can_extend_a_legacy_raw_codec_head() {
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
        assert_eq!(
            raw_cid(PROTOCOL_V4_BYTES).to_string(),
            "bafkreie4yxx5msmhvfcspqoh55v5mwt3htr7sc34yx54igg5usx2yiylie"
        );
    }
}
