use crate::{Capabilities, Mode, RadioError};
use async_trait::async_trait;

/// What a caller asks the radio to tune to.
///
/// `tx_hz` defaults to `rx_hz` for simplex; drivers that don't support split
/// return `RadioError::Unsupported` when they differ. Omit `mode` to leave
/// modulation untouched.
#[derive(Debug, Clone, Copy)]
pub struct TuneRequest {
    pub rx_hz: u64,
    pub tx_hz: u64,
    pub mode: Option<Mode>,
}

impl TuneRequest {
    pub fn simplex(hz: u64) -> Self {
        Self { rx_hz: hz, tx_hz: hz, mode: None }
    }
    pub fn split(rx_hz: u64, tx_hz: u64) -> Self {
        Self { rx_hz, tx_hz, mode: None }
    }
    pub fn with_mode(mut self, mode: Mode) -> Self {
        self.mode = Some(mode);
        self
    }
}

/// A read-only snapshot of the radio's state at a point in time. Drivers
/// return `None` for fields they can't query.
#[derive(Debug, Clone)]
pub struct RadioState {
    pub rx_hz: u64,
    pub tx_hz: u64,
    pub mode: Option<Mode>,
    pub rssi: Option<u8>,
    pub squelch_broken: Option<bool>,
    pub transmitting: bool,
    /// Driver-specific flags the server doesn't know about but can display.
    pub extras: Vec<(String, String)>,
}

/// The core driver contract. Every radio driver implements this.
///
/// **Safety invariants** (non-negotiable across every implementation):
///
/// - No RF emission except from a caller-initiated `ptt(true)`. Module
///   initialization, `new()`, `Drop`, panic, and watchdog timeout MUST leave
///   the hardware unkeyed.
/// - `tune()` quantizes to the radio's grid and returns success only after
///   the radio has acknowledged the new frequency.
/// - `ptt(false)` is always safe to call regardless of current state.
///
/// Drivers that can't meet an invariant must document the deviation
/// prominently in their crate README and fail the relevant conformance test
/// — they don't hide the limitation behind a silent `Ok(())`.
#[async_trait]
pub trait Radio: Send + Sync {
    /// Static description of what this radio can do. Should return a
    /// reference to a `Capabilities` held by the driver — cheap, no I/O.
    fn capabilities(&self) -> &Capabilities;

    /// Set RX (and optionally TX + mode). Drivers without split support
    /// return `RadioError::Unsupported` if `rx_hz != tx_hz`.
    async fn tune(&self, req: TuneRequest) -> Result<(), RadioError>;

    /// Key (true) or unkey (false) the transmitter.
    /// Only implementation path that causes RF emission.
    async fn ptt(&self, key: bool) -> Result<(), RadioError>;

    /// Read a point-in-time snapshot of the radio's current state.
    async fn read_state(&self) -> Result<RadioState, RadioError>;

    /// Send driver-specific bytes and return the response. Escape hatch
    /// for features not exposed by the trait. Drivers without a native
    /// byte-level protocol may return `RadioError::Unsupported`.
    async fn send_raw(&self, bytes: &[u8]) -> Result<Vec<u8>, RadioError>;
}
