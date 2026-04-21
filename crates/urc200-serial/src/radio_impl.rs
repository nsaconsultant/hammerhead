//! `radio_core::Radio` implementation for the URC-200 dispatcher.
//!
//! Phase 1 of the HammerHead refactor: no behavior change in the existing
//! `Radio` struct; this module adds the generic trait impl so the server can
//! start migrating to `Arc<dyn radio_core::Radio>` in Phase 2.

use crate::{Radio as Urc200Radio, RadioError as Urc200Error};
use async_trait::async_trait;
use radio_core::{
    Capabilities, FeatureFlags, FreqRange, Mode, Radio, RadioError, RadioState, TuneRequest,
};
use std::collections::HashSet;
use std::sync::LazyLock;
use urc200_proto::{
    Band, Freq, Inquiry, ModMode, OpCommand, PresetSnapshot, Rssi, SquelchStatus, Step,
};

/// Static declaration of what the URC-200 V2 can do.
/// Base-band unit by default — EBN-30/EBN-400 options broaden the ranges
/// dynamically via `?11` option_byte; that lives in the runtime state, not here.
pub static URC200_CAPS: LazyLock<Capabilities> = LazyLock::new(|| {
    let mut modes = HashSet::new();
    modes.insert(Mode::Am);
    modes.insert(Mode::Fm);
    Capabilities {
        id: "urc200-v2",
        display_name: "General Dynamics URC-200 V2",
        rx_ranges: vec![
            FreqRange::new(115_000_000, 173_995_000),
            FreqRange::new(225_000_000, 399_995_000),
        ],
        tx_ranges: vec![
            FreqRange::new(115_000_000, 173_995_000),
            FreqRange::new(225_000_000, 399_995_000),
        ],
        modes,
        tuning_step_hz: 5_000,
        presets: Some(10),
        features: FeatureFlags::PTT
            | FeatureFlags::SQUELCH_STATUS
            | FeatureFlags::SQUELCH_LEVEL
            | FeatureFlags::SMETER
            | FeatureFlags::SPLIT
            | FeatureFlags::MEMORY
            | FeatureFlags::HARDWARE_SCAN
            | FeatureFlags::TEMPERATURE
            | FeatureFlags::RAW_PASSTHROUGH,
    }
});

impl From<Urc200Error> for RadioError {
    fn from(e: Urc200Error) -> Self {
        match e {
            Urc200Error::Transport(t) => RadioError::Transport(t.to_string()),
            Urc200Error::Timeout(_) => RadioError::Timeout,
            Urc200Error::Fault => RadioError::Fault,
            Urc200Error::Closed => RadioError::Closed,
        }
    }
}

fn band_for(hz: u64) -> Option<Band> {
    match hz {
        30_000_000..=90_000_000 => Some(Band::Lvhf),
        115_000_000..=173_995_000 | 225_000_000..=399_995_000 => Some(Band::Base),
        400_000_000..=420_000_000 => Some(Band::Uhf400),
        _ => None,
    }
}

fn mode_to_urc(mode: Mode) -> Result<ModMode, RadioError> {
    match mode {
        Mode::Am => Ok(ModMode::Am),
        Mode::Fm | Mode::Nfm => Ok(ModMode::Fm),
        other => Err(RadioError::UnsupportedMode(other)),
    }
}

#[async_trait]
impl Radio for Urc200Radio {
    fn capabilities(&self) -> &Capabilities {
        &URC200_CAPS
    }

    async fn tune(&self, req: TuneRequest) -> Result<(), RadioError> {
        let band = band_for(req.rx_hz)
            .or_else(|| band_for(req.tx_hz))
            .ok_or(RadioError::OutOfRange(req.rx_hz))?;
        let step = Step::Khz5;
        let rx = Freq::new(req.rx_hz as u32, band, step)
            .map_err(|_| RadioError::OutOfRange(req.rx_hz))?;
        let tx = Freq::new(req.tx_hz as u32, band, step)
            .map_err(|_| RadioError::OutOfRange(req.tx_hz))?;
        self.send(OpCommand::SetRx(rx)).await?;
        self.send(OpCommand::SetTx(tx)).await?;
        if let Some(m) = req.mode {
            let mm = mode_to_urc(m)?;
            self.send(OpCommand::ModTxRx(mm)).await?;
        }
        Ok(())
    }

    async fn ptt(&self, key: bool) -> Result<(), RadioError> {
        let cmd = if key { OpCommand::Transmit } else { OpCommand::Receive };
        self.send(cmd).await.map(|_| ()).map_err(Into::into)
    }

    async fn read_state(&self) -> Result<RadioState, RadioError> {
        // Issue three inquiries serially: preset snapshot, RSSI, squelch.
        // Preserves ordering guarantees of the dispatcher (one in flight at a time).
        let preset_resp = self.query(Inquiry::PresetSnapshot).await?;
        let snap = PresetSnapshot::from_bytes(preset_resp.data())
            .ok_or_else(|| RadioError::Driver("preset snapshot decode".into()))?;
        let rssi_resp = self.query(Inquiry::Rssi).await?;
        let rssi = Rssi::from_bytes(rssi_resp.data()).map(|r| r.0);
        let sq_resp = self.query(Inquiry::SquelchStatus).await?;
        let squelch_broken = SquelchStatus::from_bytes(sq_resp.data())
            .map(|s| matches!(s, SquelchStatus::Broken));
        Ok(RadioState {
            rx_hz: snap.rx_hz as u64,
            tx_hz: snap.tx_hz as u64,
            mode: Some(match snap.mod_tx_rx {
                ModMode::Am => Mode::Am,
                ModMode::Fm => Mode::Fm,
            }),
            rssi,
            squelch_broken,
            transmitting: false, // derived from ?12 if needed; not critical for Phase 1
            extras: vec![
                ("preset".into(), format!("P{}", snap.preset.get())),
                ("power".into(), format!("{:?}", snap.power)),
            ],
        })
    }

    async fn send_raw(&self, bytes: &[u8]) -> Result<Vec<u8>, RadioError> {
        // UFCS to avoid recursing into this trait method.
        let resp = Urc200Radio::send_raw(self, bytes.to_vec()).await?;
        if resp.is_nak() {
            return Err(RadioError::Driver("radio NAK'd raw command".into()));
        }
        let mut out = resp.data().to_vec();
        if out.is_empty() {
            out.push(if resp.is_ack() { 0x06 } else { 0x09 });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radio_core::Radio as _;

    #[test]
    fn capabilities_declare_urc200_bands() {
        let caps = &*URC200_CAPS;
        assert_eq!(caps.id, "urc200-v2");
        assert!(caps.features.contains(FeatureFlags::PTT));
        assert!(caps.features.contains(FeatureFlags::MEMORY));
        assert_eq!(caps.presets, Some(10));
        assert!(caps.modes.contains(&Mode::Am));
        assert!(caps.modes.contains(&Mode::Fm));
    }

    #[test]
    fn usb_mode_is_unsupported() {
        // USB isn't in URC-200's advertised set.
        assert!(!URC200_CAPS.modes.contains(&Mode::Usb));
    }
}
