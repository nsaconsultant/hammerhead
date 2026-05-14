//! Library-tune funnel: route every "tune from software" through a dedicated
//! scratch preset slot so the radio's TX-time preset-recall behavior can't
//! overwrite the operator's emergency presets.
//!
//! ## Why this exists
//!
//! The URC-200 firmware re-loads the active preset's stored EEPROM contents
//! into the synth on every TX entry (`B`). If we simply set RX/TX in RAM and
//! the operator keys up, the radio reverts to whatever the active preset slot
//! has saved — undoing our tune. The fix is to make the active preset slot's
//! EEPROM match our intended tune *before* `B` arrives.
//!
//! Naive approach: `Q` after every tune. That overwrites whichever preset the
//! operator happens to be sitting on — destructive if they're using P0..=P8
//! as a fixed emergency library.
//!
//! Scratch-slot approach (this module): always switch the radio to one
//! reserved preset (the "scratch slot", default `P9`, configurable via
//! `URC_SCRATCH_PRESET`) before applying the tune, then `Q` to write the new
//! freqs to the scratch slot's EEPROM. The emergency presets at P0..=P8
//! (or wherever the scratch isn't) are never touched. The radio's TX-time
//! preset-recall is now a no-op because the scratch slot already holds what
//! we just asked for.
//!
//! ## What does NOT go through here
//!
//! - The explicit `/api/command/preset/:n` endpoint — that's the operator
//!   deliberately recalling an emergency preset, intent is to load its
//!   stored contents and use them as-is.
//! - The library scanner (`scan::tune_for_scan`) — scanning is RX-only and
//!   doing `Q` per channel-step would burn EEPROM. If the operator stops on a
//!   hit and wants to TX, they should explicitly tune the channel from the
//!   UI to push it through this funnel.

use urc200_proto::{Freq, ModMode, OpCommand, PresetId, Response};
use urc200_serial::{Radio, RadioError};

/// Outcome of one command in the sequence. Surfaced to callers so they can
/// build their HTTP response (each step gets its own ack/nak indicator).
pub struct TuneStage {
    pub command: &'static str,
    pub response: Response,
}

/// Execute the full scratch-slot tune sequence.
///
/// Sequence on the wire:
///   1. `P{scratch}` — park the radio on the scratch slot. Loads scratch's
///      stored values into RAM (about to be overwritten anyway).
///   2. `R{rx_khz}` — set RX freq in RAM.
///   3. `T{tx_khz}` — set TX freq in RAM.
///   4. `M{0|1}`    — optional mod mode in RAM.
///   5. `Q`         — persist RAM to scratch's EEPROM.
///
/// After step 5 the scratch slot's EEPROM mirrors the operator's intended
/// tune, so the radio's TX-time preset-recall is a no-op.
///
/// Returns the per-stage `Response` list. Bails on the first transport-level
/// error; NAKs from the radio do NOT abort the sequence — they're surfaced
/// in the returned vector so the caller can decide how to map them to HTTP.
pub async fn tune_via_scratch(
    radio: &Radio,
    scratch: PresetId,
    rx: Freq,
    tx: Freq,
    mode: Option<ModMode>,
) -> Result<Vec<TuneStage>, RadioError> {
    let mut out = Vec::with_capacity(5);

    let r = radio.send(OpCommand::Preset(scratch)).await?;
    out.push(TuneStage { command: "preset", response: r });

    let r = radio.send(OpCommand::SetRx(rx)).await?;
    out.push(TuneStage { command: "rx", response: r });

    let r = radio.send(OpCommand::SetTx(tx)).await?;
    out.push(TuneStage { command: "tx", response: r });

    if let Some(m) = mode {
        let r = radio.send(OpCommand::ModTxRx(m)).await?;
        out.push(TuneStage { command: "mode", response: r });
    }

    let r = radio.send(OpCommand::StorePreset).await?;
    out.push(TuneStage { command: "store", response: r });

    Ok(out)
}

/// Pick a `PresetId` from the `URC_SCRATCH_PRESET` env var (default `9`).
/// Falls back to `P9` for any parse / range error so the server still starts.
pub fn scratch_preset_from_env() -> PresetId {
    let raw = std::env::var("URC_SCRATCH_PRESET").unwrap_or_else(|_| "9".to_string());
    raw.parse::<u8>()
        .ok()
        .and_then(PresetId::new)
        .unwrap_or_else(|| {
            tracing::warn!(
                value = %raw,
                "URC_SCRATCH_PRESET must be 0..=9; falling back to P9"
            );
            PresetId::new(9).expect("9 is a valid PresetId")
        })
}
