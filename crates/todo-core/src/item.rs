use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use thiserror::Error;
use uuid::Uuid;

/// Stable identity for an item. Titles and project membership may change
/// without changing this identity. Source: DI-0002.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ItemId(Uuid);

impl ItemId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(value: &str) -> Result<Self, uuid::Error> {
        Uuid::parse_str(value).map(Self)
    }
}

impl Default for ItemId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// User-facing lifecycle. `Ongoing` is deliberately a CLI alias rather than a
/// second stored state. Source: DI-0001; DI-0004.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ItemState {
    Inbox,
    Active,
    Done,
    Canceled,
}

impl ItemState {
    pub fn can_transition_to(self, next: Self) -> bool {
        match (self, next) {
            (current, candidate) if current == candidate => true,
            (Self::Inbox, Self::Active | Self::Done | Self::Canceled) => true,
            (Self::Active, Self::Inbox | Self::Done | Self::Canceled) => true,
            (Self::Done | Self::Canceled, Self::Active) => true,
            _ => false,
        }
    }
}

impl fmt::Display for ItemState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Inbox => "inbox",
            Self::Active => "active",
            Self::Done => "done",
            Self::Canceled => "canceled",
        };
        formatter.write_str(value)
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
#[error("unknown item state {0:?}; expected inbox, active, ongoing, done, or canceled")]
pub struct ParseItemStateError(String);

impl FromStr for ItemState {
    type Err = ParseItemStateError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "inbox" => Ok(Self::Inbox),
            "active" | "ongoing" => Ok(Self::Active),
            "done" => Ok(Self::Done),
            "canceled" | "cancelled" => Ok(Self::Canceled),
            _ => Err(ParseItemStateError(value.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    pub title: String,
    pub state: ItemState,
    pub project_id: Option<NamespaceId>,
    pub parent_id: Option<ItemId>,
    pub priority: Priority,
    pub due_on: Option<NaiveDate>,
    pub defer_until: Option<NaiveDate>,
    pub scheduled_on: Option<NaiveDate>,
    /// How hard the work is, from 1 to 5. Independent of how long it takes.
    pub difficulty: Option<u8>,
    /// Expected effort in minutes.
    pub estimate_minutes: Option<u32>,
}

use crate::NamespaceId;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

impl Priority {
    pub fn rank(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Normal => 1,
            Self::High => 2,
            Self::Critical => 3,
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Critical => "critical",
        })
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
#[error("unknown priority {0:?}; expected low/p3, normal/p2, high/p1, or critical/p0")]
pub struct ParsePriorityError(String);

impl FromStr for Priority {
    type Err = ParsePriorityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "low" | "p3" => Ok(Self::Low),
            "normal" | "p2" => Ok(Self::Normal),
            "high" | "p1" => Ok(Self::High),
            "critical" | "p0" => Ok(Self::Critical),
            _ => Err(ParsePriorityError(value.to_owned())),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "test fixtures use direct setup and assertion values"
)]
mod tests {
    use super::{ItemState, Priority};

    #[test]
    fn ongoing_is_an_active_alias() {
        assert_eq!("ongoing".parse(), Ok(ItemState::Active));
        assert_eq!(ItemState::Active.to_string(), "active");
    }

    #[test]
    fn completed_items_reopen_to_active_but_not_inbox_and_active_can_be_parked() {
        assert!(ItemState::Done.can_transition_to(ItemState::Active));
        assert!(!ItemState::Done.can_transition_to(ItemState::Inbox));
        assert!(ItemState::Active.can_transition_to(ItemState::Inbox));
    }

    #[test]
    fn priority_aliases_have_an_explicit_order() {
        assert_eq!("p0".parse(), Ok(Priority::Critical));
        assert!(Priority::Critical.rank() > Priority::High.rank());
        assert!(Priority::High.rank() > Priority::Normal.rank());
    }
}
