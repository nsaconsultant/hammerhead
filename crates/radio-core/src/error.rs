use crate::Mode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadioError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("no response within timeout")]
    Timeout,
    #[error("protocol fault — driver gave up after repeated errors")]
    Fault,
    #[error("radio handle closed")]
    Closed,
    #[error("mode {0:?} not supported by this radio")]
    UnsupportedMode(Mode),
    #[error("frequency {0} Hz outside any supported range")]
    OutOfRange(u64),
    #[error("feature not supported by this radio")]
    Unsupported,
    #[error("driver error: {0}")]
    Driver(String),
}
