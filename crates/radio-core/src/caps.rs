use crate::Mode;
use bitflags::bitflags;
use std::collections::HashSet;

/// Inclusive frequency range in Hz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FreqRange {
    pub start_hz: u64,
    pub end_hz: u64,
}

impl FreqRange {
    pub const fn new(start_hz: u64, end_hz: u64) -> Self {
        Self { start_hz, end_hz }
    }
    pub fn contains(&self, hz: u64) -> bool {
        (self.start_hz..=self.end_hz).contains(&hz)
    }
}

bitflags! {
    /// Per-driver capability flags — what the radio can actually do beyond
    /// basic tune/PTT/read. Used by the server to decide which UI affordances
    /// to render, which HTTP routes to enable, and which conformance tests
    /// to skip.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FeatureFlags: u32 {
        /// Radio has a transmit path we can key (false for RX-only receivers).
        const PTT             = 1 << 0;
        /// Radio reports squelch open/closed state.
        const SQUELCH_STATUS  = 1 << 1;
        /// Radio has a configurable squelch level (not just open/closed).
        const SQUELCH_LEVEL   = 1 << 2;
        /// Radio reports signal strength (S-meter).
        const SMETER          = 1 << 3;
        /// Radio has internal CTCSS encode support (bypasses software mix).
        const CTCSS_ENCODE    = 1 << 4;
        /// Radio has internal CTCSS decode support (tone squelch).
        const CTCSS_DECODE    = 1 << 5;
        /// Split TX/RX (different frequencies on each).
        const SPLIT           = 1 << 6;
        /// Memory/preset slots.
        const MEMORY          = 1 << 7;
        /// Scan mode in the radio's own firmware.
        const HARDWARE_SCAN   = 1 << 8;
        /// Reports internal temperature / over-temp.
        const TEMPERATURE     = 1 << 9;
        /// Reports supply voltage rails.
        const VOLTAGE         = 1 << 10;
        /// Accepts arbitrary bytes via send_raw (most do).
        const RAW_PASSTHROUGH = 1 << 11;
    }
}

/// What a driver declares about its radio. Static data read once per driver
/// instance; drivers should return `&self.caps` from their `capabilities()`.
#[derive(Debug, Clone)]
pub struct Capabilities {
    /// Driver identifier — `"urc200-v2"`, `"prc117f"`, `"ic7300"`. Used in
    /// config and log messages. Lowercase kebab-case.
    pub id: &'static str,
    /// Human-readable model name — `"General Dynamics URC-200 V2"`.
    pub display_name: &'static str,
    /// Receive ranges (may be disjoint — e.g. URC-200 is 115-174 + 225-400).
    pub rx_ranges: Vec<FreqRange>,
    /// Transmit ranges (may be narrower than RX, or empty for receivers).
    pub tx_ranges: Vec<FreqRange>,
    /// Modes the radio can be tuned to.
    pub modes: HashSet<Mode>,
    /// Smallest native frequency step, in Hz. Drivers quantize requested
    /// frequencies to this grid.
    pub tuning_step_hz: u32,
    /// Number of native memory slots (e.g. URC-200 has 10). None = no presets.
    pub presets: Option<u8>,
    /// Capability flags — optional features that may or may not be present.
    pub features: FeatureFlags,
}
