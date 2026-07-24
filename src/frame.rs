use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Frame {
    pub bytes: Vec<u8>,
    pub timestamp: u128,
    pub hash: u64,
}
impl PartialEq for Frame {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Frame {
    pub fn new(bytes: &[u8]) -> Self {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        let hash = hasher.finish();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        Self {
            bytes: bytes.to_vec(),
            timestamp,
            hash,
        }
    }

    pub fn is_duplicate_of(&self, other: &Frame) -> bool {
        self.hash == other.hash
    }

    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
    pub fn decode(bytes: &[u8]) -> Result<Frame, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
