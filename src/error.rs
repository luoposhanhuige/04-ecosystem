use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("An I/O error occurred: {0}")]
    Io(#[from] std::io::Error),
    #[error("A parsing error occurred: {0}")]
    // Parse(#[from] std::str::Utf8Error),
    Parse(#[from] std::num::ParseIntError),
    #[error("A serialization json error occurred: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("A custom error occurred: {0}")]
    Custom(String),
}
