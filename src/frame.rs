use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use xxhash_rust::const_xxh3;

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
        let hash = const_xxh3::xxh3_64(bytes);

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

    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
    pub fn decode(bytes: &[u8]) -> Result<Frame, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
