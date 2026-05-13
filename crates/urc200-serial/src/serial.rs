use crate::{Transport, TransportError};
use async_trait::async_trait;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serial::{
    ClearBuffer, DataBits, FlowControl, Parity, SerialPort, SerialPortBuilderExt, SerialStream,
    StopBits,
};

/// Serial-port settings. Defaults match URC-200 §4.6.2: 1200 bps, 8-N-1, no flow control.
#[derive(Debug, Clone)]
pub struct SerialConfig {
    pub path: String,
    pub baud: u32,
    pub data_bits: DataBits,
    pub stop_bits: StopBits,
    pub parity: Parity,
    pub flow_control: FlowControl,
    pub open_timeout: Duration,
}

impl SerialConfig {
    pub fn urc200(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            baud: 1200,
            data_bits: DataBits::Eight,
            stop_bits: StopBits::One,
            parity: Parity::None,
            flow_control: FlowControl::None,
            open_timeout: Duration::from_millis(100),
        }
    }
}

/// Real `tokio-serial`-backed [`Transport`].
pub struct SerialTransport {
    stream: SerialStream,
}

impl SerialTransport {
    /// Open a serial port configured for the URC-200.
    pub fn open(cfg: &SerialConfig) -> Result<Self, TransportError> {
        let mut stream = tokio_serial::new(&cfg.path, cfg.baud)
            .data_bits(cfg.data_bits)
            .stop_bits(cfg.stop_bits)
            .parity(cfg.parity)
            .flow_control(cfg.flow_control)
            .timeout(cfg.open_timeout)
            .open_native_async()?;
        // Discard any bytes left in the OS buffers from a prior session.
        // Stale RX bytes were the proximate cause of an observed dispatcher
        // off-by-one wedge — one late response from before the prior crash
        // would land in the kernel buffer and get attributed to the next
        // command after restart, then every subsequent response too. The
        // companion `drain_input` call in the dispatcher loop handles the
        // steady-state case; this open-time clear handles the cold start.
        if let Err(e) = stream.clear(ClearBuffer::All) {
            tracing::warn!(error = ?e, "clear ClearBuffer::All on open failed; continuing");
        }
        Ok(Self { stream })
    }
}

#[async_trait]
impl Transport for SerialTransport {
    async fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.stream.write_all(bytes).await?;
        self.stream.flush().await?;
        Ok(())
    }

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
        let n = self.stream.read(buf).await?;
        Ok(n)
    }

    async fn drain_input(&mut self) -> Result<(), TransportError> {
        // `clear` is a synchronous ioctl on the underlying fd; safe to call
        // from an async context. Returns serialport::Error → tokio_serial::Error.
        self.stream.clear(ClearBuffer::Input)?;
        Ok(())
    }
}
