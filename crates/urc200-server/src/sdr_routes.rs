//! SDR waterfall routes — only compiled when the `sdr` Cargo feature is on.
//!
//! Exposes:
//!   * `/api/ws/waterfall` — binary framed spectrum feed. Each frame is a
//!     16-byte header (u64 center_hz LE, u32 sample_rate LE, u32 n_bins LE)
//!     followed by `n_bins` bytes of u8 magnitude. Self-describing so the
//!     client doesn't need to coordinate retunes with a separate channel.
//!   * `/api/sdr/config` — GET returns current tune; POST retunes / changes
//!     gain at runtime.
//!
//! The capture thread is owned by `SdrCapture` and spawned once at startup.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use radio_sdr::{SdrCapture, SdrConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::AppState;

#[derive(Clone)]
pub struct SdrHandle {
    pub capture: Arc<SdrCapture>,
}

impl SdrHandle {
    pub fn spawn(cfg: SdrConfig) -> Self {
        Self {
            capture: Arc::new(SdrCapture::spawn(cfg)),
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/ws/waterfall", get(ws_waterfall))
        .route("/api/sdr/config", get(get_config).post(set_config))
}

#[derive(Serialize)]
struct SdrConfigSnapshot {
    center_hz: u64,
    sample_rate: u32,
    fft_size: usize,
}

#[derive(Deserialize)]
struct SdrConfigReq {
    center_hz: Option<u64>,
    gain_db: Option<f64>,
    /// Pass `"auto"` to re-enable AGC; any numeric value sets fixed gain.
    gain_mode: Option<String>,
}

async fn get_config(State(s): State<AppState>) -> Json<SdrConfigSnapshot> {
    let sdr = &s.sdr.capture;
    Json(SdrConfigSnapshot {
        center_hz: sdr.center_hz(),
        sample_rate: sdr.sample_rate(),
        fft_size: sdr.fft_size(),
    })
}

async fn set_config(
    State(s): State<AppState>,
    Json(req): Json<SdrConfigReq>,
) -> Json<SdrConfigSnapshot> {
    let sdr = &s.sdr.capture;
    if let Some(hz) = req.center_hz {
        sdr.set_center(hz).await;
    }
    match req.gain_mode.as_deref() {
        Some("auto") => sdr.set_gain(None).await,
        _ => {
            if let Some(g) = req.gain_db {
                sdr.set_gain(Some(g)).await;
            }
        }
    }
    Json(SdrConfigSnapshot {
        center_hz: sdr.center_hz(),
        sample_rate: sdr.sample_rate(),
        fft_size: sdr.fft_size(),
    })
}

async fn ws_waterfall(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle(socket, state.sdr))
        .into_response()
}

async fn handle(mut socket: WebSocket, sdr: SdrHandle) {
    use tokio::sync::broadcast::error::RecvError;
    let mut rx = sdr.capture.subscribe();

    // Text preamble — lets the client pick a reasonable canvas aspect without
    // waiting on the first binary frame.
    let hdr = serde_json::json!({
        "type": "sdr_header",
        "center_hz": sdr.capture.center_hz(),
        "sample_rate": sdr.capture.sample_rate(),
        "fft_size": sdr.capture.fft_size(),
    });
    if socket.send(Message::Text(hdr.to_string())).await.is_err() {
        return;
    }
    info!("waterfall ws client connected");

    loop {
        tokio::select! {
            frame = rx.recv() => match frame {
                Ok(f) => {
                    // 16-byte self-describing preamble, then bins.
                    let n = f.bins.len();
                    let mut payload = Vec::with_capacity(16 + n);
                    payload.extend_from_slice(&f.center_hz.to_le_bytes());
                    payload.extend_from_slice(&f.sample_rate.to_le_bytes());
                    payload.extend_from_slice(&(n as u32).to_le_bytes());
                    payload.extend_from_slice(&f.bins);
                    if socket.send(Message::Binary(payload)).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            },
            incoming = socket.recv() => match incoming {
                None => break,
                Some(Err(_)) => break,
                Some(Ok(Message::Close(_))) => break,
                // Accept client text messages as retune requests: {"center_hz": 251950000}
                Some(Ok(Message::Text(text))) => {
                    if let Ok(req) = serde_json::from_str::<SdrConfigReq>(&text) {
                        if let Some(hz) = req.center_hz {
                            sdr.capture.set_center(hz).await;
                        }
                        match req.gain_mode.as_deref() {
                            Some("auto") => sdr.capture.set_gain(None).await,
                            _ => if let Some(g) = req.gain_db {
                                sdr.capture.set_gain(Some(g)).await;
                            }
                        }
                    }
                }
                Some(Ok(_)) => {}
            }
        }
    }
    info!("waterfall ws client disconnected");
}
