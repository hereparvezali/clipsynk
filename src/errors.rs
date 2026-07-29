use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum AppErr {
    #[error("ReadErr: {0}")]
    ReadErr(String),
    #[error("WrieErr: {0}")]
    WriteErr(String),
    #[error("Deserialize: {0}")]
    Deserialize(String),
    #[error("Serialize: {0}")]
    SerializeErr(String),
    #[error("AddressErr")]
    AddressErr,

    #[error("AlreadyExistErr: {0}")]
    AlreadyExistErr(String),
}
