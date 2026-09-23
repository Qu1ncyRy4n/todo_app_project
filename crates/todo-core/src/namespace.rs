use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct NamespaceId(Uuid);

impl NamespaceId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(value: &str) -> Result<Self, uuid::Error> {
        Uuid::parse_str(value).map(Self)
    }
}

impl Default for NamespaceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NamespaceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NamespaceKind {
    Tag,
    Project,
    Role,
    Goal,
    Topic,
    Context,
    Custom(String),
}

impl fmt::Display for NamespaceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tag => formatter.write_str("tag"),
            Self::Project => formatter.write_str("project"),
            Self::Role => formatter.write_str("role"),
            Self::Goal => formatter.write_str("goal"),
            Self::Topic => formatter.write_str("topic"),
            Self::Context => formatter.write_str("context"),
            Self::Custom(value) => formatter.write_str(value),
        }
    }
}

impl FromStr for NamespaceKind {
    type Err = NamespacePathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validate_segment(value)?;
        Ok(match value {
            "tag" => Self::Tag,
            "project" => Self::Project,
            "role" => Self::Role,
            "goal" => Self::Goal,
            "topic" => Self::Topic,
            "context" => Self::Context,
            other => Self::Custom(other.to_owned()),
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct NamespacePath(String);

impl NamespacePath {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_self_or_descendant_of(&self, ancestor: &Self) -> bool {
        self == ancestor
            || self
                .0
                .strip_prefix(&ancestor.0)
                .is_some_and(|suffix| suffix.starts_with('/'))
    }

    pub fn kind(&self) -> Result<NamespaceKind, NamespacePathError> {
        self.0
            .split('/')
            .next()
            .ok_or(NamespacePathError::MissingName)?
            .parse()
    }
}

impl fmt::Display for NamespacePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum NamespacePathError {
    #[error("namespace path must contain at least a kind and name, for example project/todo")]
    MissingName,
    #[error("invalid namespace segment {0:?}; use lowercase letters, digits, dash, or underscore")]
    InvalidSegment(String),
}

impl FromStr for NamespacePath {
    type Err = NamespacePathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let segments: Vec<_> = value.split('/').collect();
        if segments.len() < 2 {
            return Err(NamespacePathError::MissingName);
        }
        for segment in segments {
            validate_segment(segment)?;
        }
        Ok(Self(value.to_owned()))
    }
}

fn validate_segment(segment: &str) -> Result<(), NamespacePathError> {
    if segment.is_empty()
        || !segment.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
    {
        return Err(NamespacePathError::InvalidSegment(segment.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "test fixtures use direct setup and assertion values"
)]
mod tests {
    use super::NamespacePath;

    #[test]
    fn prefix_matching_respects_segment_boundaries() {
        let root: NamespacePath = "project/lab".parse().unwrap();
        assert!(
            "project/lab/cclab"
                .parse::<NamespacePath>()
                .unwrap()
                .is_self_or_descendant_of(&root)
        );
        assert!(
            !"project/labor"
                .parse::<NamespacePath>()
                .unwrap()
                .is_self_or_descendant_of(&root)
        );
    }

    #[test]
    fn paths_require_kind_and_name() {
        assert!("project".parse::<NamespacePath>().is_err());
        assert!("Project/todo".parse::<NamespacePath>().is_err());
    }
}
