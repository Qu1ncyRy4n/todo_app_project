use crate::{ItemId, NamespaceId};
use chrono::{DateTime, Utc};
use cid::Cid;
use minicbor::{Encoder, data::Tag};
use multihash::Multihash;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt};
use thiserror::Error;
use uuid::Uuid;

const GRID_TAG: u64 = 1_735_551_332;
const CID_TAG: u64 = 42;
const PROTOCOL_V1_BYTES: &[u8] = include_bytes!("../../../docs/protocols/todo-ledger-event-v1.md");
const PROTOCOL_V2_BYTES: &[u8] = include_bytes!("../../../docs/protocols/todo-ledger-event-v2.md");
const PROTOCOL_V3_BYTES: &[u8] = include_bytes!("../../../docs/protocols/todo-ledger-event-v3.md");

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
    ProjectPathAttached,
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
            Self::ProjectPathAttached => "project.path-attached",
        })
    }
}

impl EventKind {
    fn protocol_bytes(&self) -> &'static [u8] {
        match self {
            Self::ItemPriorityChanged
            | Self::ItemDueDateChanged
            | Self::ItemDeferDateChanged
            | Self::ItemDependencyAdded
            | Self::ItemDependencyRemoved => PROTOCOL_V2_BYTES,
            Self::ItemTitleChanged
            | Self::ItemParentCleared
            | Self::ItemProjectChanged
            | Self::NamespaceRemoved => PROTOCOL_V3_BYTES,
            _ => PROTOCOL_V1_BYTES,
        }
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
        let protocol_cid = cid_for(self.payload.kind.protocol_bytes());
        let bytes = encode_envelope(&protocol_cid, self.parent.as_ref(), &self.payload)?;
        let cid = cid_for(&bytes);
        Ok(Event {
            cid,
            parent: self.parent,
            payload: self.payload,
            bytes,
        })
    }
}

#[derive(Debug, Error)]
#[error("encode event: {0}")]
pub struct EventEncodeError(#[from] minicbor::encode::Error<std::convert::Infallible>);

fn cid_for(bytes: &[u8]) -> Cid {
    let digest = Sha256::digest(bytes);
    let hash = Multihash::<64>::wrap(0x12, &digest).expect("SHA-256 fits the CID digest");
    Cid::new_v1(0x55, hash)
}

fn encode_envelope(
    protocol_cid: &Cid,
    parent: Option<&Cid>,
    payload: &EventPayload,
) -> Result<Vec<u8>, EventEncodeError> {
    let mut encoder = Encoder::new(Vec::new());
    encoder.tag(Tag::new(GRID_TAG))?.array(4)?;
    encode_cid(&mut encoder, protocol_cid)?;

    encoder.array(u64::from(parent.is_some()))?;
    if let Some(parent) = parent {
        encode_cid(&mut encoder, parent)?;
    }

    encoder.map(10)?;
    encoder.str("schema_version")?.u8(1)?;
    encoder.str("action")?.str("record")?;
    encoder.str("kind")?.str(&payload.kind.to_string())?;
    encoder
        .str("event_id")?
        .str(&payload.event_id.to_string())?;
    encoder.str("actor_id")?.str(&payload.actor_id)?;
    encoder.str("subject_id")?.str(&payload.subject_id)?;
    encoder.str("project_id")?;
    match payload.project_id {
        Some(project_id) => encoder.str(&project_id.to_string())?,
        None => encoder.null()?,
    };
    encoder
        .str("recorded_at")?
        .str(&payload.recorded_at.to_rfc3339())?;
    encoder.str("data")?.map(payload.data.len() as u64)?;
    for (key, value) in &payload.data {
        encoder.str(key)?.str(value)?;
    }
    encoder.str("links")?.array(payload.links.len() as u64)?;
    for link in &payload.links {
        encoder
            .map(2)?
            .str("relation")?
            .str(&link.relation)?
            .str("target")?
            .str(&link.target)?;
    }
    encoder.null()?;
    Ok(encoder.into_writer())
}

fn encode_cid(
    encoder: &mut Encoder<Vec<u8>>,
    cid: &Cid,
) -> Result<(), minicbor::encode::Error<std::convert::Infallible>> {
    let mut content = Vec::with_capacity(cid.encoded_len() + 1);
    content.push(0);
    content.extend_from_slice(&cid.to_bytes());
    encoder.tag(Tag::new(CID_TAG))?.bytes(&content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        EventBuilder, EventKind, GRID_TAG, PROTOCOL_V1_BYTES, PROTOCOL_V2_BYTES, PROTOCOL_V3_BYTES,
        cid_for,
    };
    use crate::ItemId;
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
    fn protocol_versions_have_distinct_content_identities() {
        assert_eq!(
            cid_for(PROTOCOL_V1_BYTES).to_string(),
            "bafkreieeg4aciqnmr2eze5zq5yxmcweoi5zropjjdir7vultwijcsluumq"
        );
        assert_eq!(
            cid_for(PROTOCOL_V2_BYTES).to_string(),
            "bafkreib343dtizusqbavpevtkbfxmqdqshfl2tgd2uq7iqg2boi2pldlha"
        );
        assert_ne!(cid_for(PROTOCOL_V2_BYTES), cid_for(PROTOCOL_V3_BYTES));
    }
}
