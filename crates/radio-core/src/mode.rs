use serde::{Deserialize, Serialize};

/// Modulation mode expressed as a closed enum over the real-world set.
///
/// The design choice is deliberate: not a string, not an open enum, not
/// `Box<dyn Trait>`. A closed enum gives us exhaustive match checking; new
/// digital protocols slot into [`DigitalMode`] without polluting the core set.
///
/// Drivers advertise the modes they support via `Capabilities::modes` and
/// reject anything else at `tune()` time with `RadioError::UnsupportedMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Amplitude modulation.
    Am,
    /// Frequency modulation (general / wideband FM broadcast-width).
    Fm,
    /// Narrowband FM (2.5 kHz deviation — ham/mil voice).
    Nfm,
    /// Wideband FM (broadcast / satellite).
    Wfm,
    /// Upper side-band.
    Usb,
    /// Lower side-band.
    Lsb,
    /// CW (Morse).
    Cw,
    /// CW reverse side-band.
    CwR,
    /// RTTY.
    Rtty,
    /// RTTY reverse.
    RttyR,
    /// Digital protocol carried over audio (FT8, DMR, P25, etc.).
    Digital(DigitalMode),
}

/// Digital protocols that sit on top of an audio pipe. Hamlib maps several
/// of these to a generic `DIGITAL` mode; we retain the distinction so
/// drivers that DO have a specific mode (DMR radios, P25 radios) can set it
/// natively, and software-only digital (FT8 via WSJT-X through a soundcard)
/// can be carried as `Digital(FT8)` over the audio path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DigitalMode {
    Ft8,
    Ft4,
    Js8,
    Psk31,
    Psk63,
    Olivia,
    Mfsk,
    Rtty,
    Dmr,
    P25,
    DStar,
    C4fm,
    /// Catch-all for driver-defined digital modes that don't map cleanly.
    Vendor,
}
