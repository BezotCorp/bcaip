use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use bincode_next::{Decode, Encode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::{Uuid, Version};

#[derive(Clone, Copy, Debug, Decode, Encode, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutableId([u8; 16]);

impl ExecutableId {
    pub fn new() -> Self {
        Self(Uuid::now_v7().into_bytes())
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        let uuid =
            Uuid::parse_str(value).map_err(|error| format!("invalid executable UUID: {error}"))?;

        if uuid.get_version() != Some(Version::SortRand) {
            return Err(format!("executable UUID must be UUIDv7: {uuid}"));
        }

        Ok(Self(uuid.into_bytes()))
    }

    pub fn as_uuid(self) -> Uuid {
        Uuid::from_bytes(self.0)
    }
}

impl Default for ExecutableId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for ExecutableId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        self.as_uuid().fmt(formatter)
    }
}

impl FromStr for ExecutableId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for ExecutableId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ExecutableId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;

        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}
