use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;
use xxhash_rust::const_xxh3;

use crate::errors::AppErr;

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

    pub async fn read<R>(r: &mut R) -> Result<Self, AppErr>
    where
        R: AsyncReadExt + Unpin,
    {
        let mut len_buf = [0_u8; 4];
        if r.read_exact(&mut len_buf).await.is_err() {
            return Err(AppErr::ReadErr("Frame lenght reading error".into()));
        };
        let n = u32::from_be_bytes(len_buf) as usize;
        let mut buf = vec![0u8; n];
        r.read_exact(&mut buf)
            .await
            .map_err(|_| AppErr::ReadErr("Reading exact Frame".into()))?;
        let frame =
            Self::decode(&buf).map_err(|_| AppErr::Deserialize("Frame decode err".into()))?;
        Ok(frame)
    }
    pub async fn write<W>(&self, w: &mut W) -> Result<(), AppErr>
    where
        W: AsyncWriteExt + Unpin,
    {
        let bytes = self
            .encode()
            .map_err(|_| AppErr::SerializeErr("Frame serialize err".into()))?;
        let n = (bytes.len() as u32).to_be_bytes();
        if w.write_all(&n).await.is_err() || w.write_all(&bytes).await.is_err() {
            return Err(AppErr::WriteErr("Frame Writting Err".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HandShake {
    pub device_id: Uuid,
    pub tcp_port: u16,
}
impl HandShake {
    pub fn new(device_id: Uuid, tcp_port: u16) -> Self {
        Self {
            device_id,
            tcp_port,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    pub async fn read<R>(r: &mut R) -> Result<Self, AppErr>
    where
        R: AsyncReadExt + Unpin,
    {
        let mut len_buf = [0_u8; 4];
        if r.read_exact(&mut len_buf).await.is_err() {
            return Err(AppErr::ReadErr("HandShake lenght reading error".into()));
        };
        let n = u32::from_be_bytes(len_buf) as usize;
        let mut buf = vec![0u8; n];
        r.read_exact(&mut buf)
            .await
            .map_err(|_| AppErr::ReadErr("Reading exact handshake".into()))?;
        let handshake =
            Self::decode(&buf).map_err(|_| AppErr::Deserialize("Handshake decode err".into()))?;
        Ok(handshake)
    }
    pub async fn write<W>(&self, w: &mut W) -> Result<(), AppErr>
    where
        W: AsyncWriteExt + Unpin,
    {
        let bytes = self
            .encode()
            .map_err(|_| AppErr::SerializeErr("Handshake serialize err".into()))?;
        let n = (bytes.len() as u32).to_be_bytes();
        if w.write_all(&n).await.is_err() || w.write_all(&bytes).await.is_err() {
            return Err(AppErr::WriteErr("HandShake Writting Err".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_hashes() {
        let cases = ["", "hello", "ClipSynk", "The quick brown fox jumps over the lazy dog"];
        for s in cases {
            let h = const_xxh3::xxh3_64(s.as_bytes());
            let f = Frame::new(s.as_bytes());
            let json = serde_json::to_string(&f).unwrap();
            println!("HASH for '{}': {} (hex: 0x{:016x}), JSON: {}", s, h, h, json);
        }
    }
}

