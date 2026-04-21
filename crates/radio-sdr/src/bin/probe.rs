//! CLI smoke test for `radio-sdr`. Opens the SDR, consumes spectrum frames
//! for 10 seconds, prints a summary. Useful for verifying SoapySDR / SDRplay
//! plumbing before wiring the library into the main server.
//!
//! Usage:
//!   cargo run -p radio-sdr --bin radio-sdr-probe
//!   cargo run -p radio-sdr --bin radio-sdr-probe -- --center 147000000

use anyhow::Result;
use radio_sdr::{SdrCapture, SdrConfig};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "radio_sdr=info".into()),
        )
        .init();

    let mut cfg = SdrConfig::default();
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--center" => {
                cfg.center_hz = args[i + 1].parse()?;
                i += 2;
            }
            "--rate" => {
                cfg.sample_rate = args[i + 1].parse()?;
                i += 2;
            }
            "--fft" => {
                cfg.fft_size = args[i + 1].parse()?;
                i += 2;
            }
            "--device" => {
                cfg.device_args = args[i + 1].clone();
                i += 2;
            }
            _ => i += 1,
        }
    }

    println!("opening SDR: device={} center={} Hz rate={} Hz fft={}",
        cfg.device_args, cfg.center_hz, cfg.sample_rate, cfg.fft_size);
    let sdr = SdrCapture::spawn(cfg.clone());
    let mut rx = sdr.subscribe();

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut frames = 0u64;
    let mut peak_any = 0u8;
    let mut first_ts: Option<u64> = None;
    let mut last_ts: Option<u64> = None;

    while Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            Ok(Ok(frame)) => {
                frames += 1;
                if first_ts.is_none() {
                    first_ts = Some(frame.timestamp_ms);
                }
                last_ts = Some(frame.timestamp_ms);
                let p = *frame.bins.iter().max().unwrap_or(&0);
                if p > peak_any {
                    peak_any = p;
                }
                if frames == 1 {
                    println!("first frame: {} bins, center={} Hz", frame.bins.len(), frame.center_hz);
                }
            }
            Ok(Err(_)) => break,
            Err(_) => {
                // timeout tick; keep waiting
            }
        }
    }

    sdr.shutdown().await;

    let elapsed_ms = match (first_ts, last_ts) {
        (Some(a), Some(b)) if b > a => b - a,
        _ => 0,
    };
    let rate = if elapsed_ms > 0 {
        (frames as f64) * 1000.0 / elapsed_ms as f64
    } else {
        0.0
    };

    println!("done · frames={} elapsed={} ms avg={:.1} fps peak_bin={}",
        frames, elapsed_ms, rate, peak_any);
    Ok(())
}
