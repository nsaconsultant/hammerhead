//! `radio-core` — the generic Radio trait and the shared types every driver
//! in the HammerHead workspace speaks.
//!
//! Drivers live in their own crates under `crates/drivers/<name>/` and
//! implement [`Radio`]. The server never names a concrete driver — it loads
//! them by id from config and dispatches through `Arc<dyn Radio>`.
//!
//! This crate is deliberately I/O-free. No tokio, no ALSA, no HTTP — just
//! the contract.

pub mod caps;
pub mod error;
pub mod mode;
pub mod radio;

pub use caps::{Capabilities, FeatureFlags, FreqRange};
pub use error::RadioError;
pub use mode::{DigitalMode, Mode};
pub use radio::{Radio, RadioState, TuneRequest};
