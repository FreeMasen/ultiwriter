//! Direct USB access to the box's FT232BM bridge via `nusb` (pure Rust; no
//! libusb or FTDI driver).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use nusb::transfer::{Buffer, Bulk, ControlOut, ControlType, In, Out, Recipient, TransferError};
use nusb::{Endpoint, Interface, MaybeFuture};

use crate::client::{Error, Port, Result};

pub const VID: u16 = 0x0403;
/// Single-slot ("Reader/Writer Box") and dual-slot boxes.
pub const PIDS: [u16; 2] = [0xF620, 0xF621];

const EP_IN: u8 = 0x81;
const EP_OUT: u8 = 0x02;
/// FT232BM full-speed packet; each IN packet starts with 2 modem-status bytes.
const PACKET: usize = 64;
/// One IN transfer: up to 64 packets, ending early on a short packet.
const IN_TRANSFER: usize = PACKET * 64;
const CONTROL_TIMEOUT: Duration = Duration::from_secs(1);
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

// FTDI SIO vendor requests.
const SIO_RESET: u8 = 0;
const SIO_SET_FLOW_CTRL: u8 = 2;
const SIO_SET_BAUDRATE: u8 = 3;
const SIO_SET_DATA: u8 = 4;
const SIO_SET_LATENCY: u8 = 9;
const RESET_SIO: u16 = 0;
const PURGE_RX: u16 = 1;
const PURGE_TX: u16 = 2;
/// Interface A, as wIndex for per-port requests.
const PORT_A: u16 = 1;

impl From<nusb::Error> for Error {
    fn from(e: nusb::Error) -> Self {
        Error::Usb(e.to_string())
    }
}

impl From<TransferError> for Error {
    fn from(e: TransferError) -> Self {
        Error::Usb(e.to_string())
    }
}

pub struct FtdiPort {
    interface: Interface,
    ep_in: Endpoint<Bulk, In>,
    ep_out: Endpoint<Bulk, Out>,
    rx: VecDeque<u8>,
    product: Option<String>,
    pub product_id: u16,
}

impl FtdiPort {
    /// Open the first attached box and configure the line like the
    /// Windows app's driver defaults: 9600 8N1, no flow control.
    pub fn open() -> Result<Self> {
        let info = nusb::list_devices()
            .wait()?
            .find(|d| d.vendor_id() == VID && PIDS.contains(&d.product_id()))
            .ok_or(Error::NoDevice)?;
        let device = info.open().wait()?;
        let interface = device.detach_and_claim_interface(0).wait()?;
        let mut p = FtdiPort {
            ep_in: interface.endpoint::<Bulk, In>(EP_IN)?,
            ep_out: interface.endpoint::<Bulk, Out>(EP_OUT)?,
            interface,
            rx: VecDeque::new(),
            product: info.product_string().map(str::to_string),
            product_id: info.product_id(),
        };
        p.control(SIO_RESET, RESET_SIO, PORT_A)?;
        // 3 MHz / 9600 = 312.5: divisor 312, fraction code 1 (0.5) in bits 14-15.
        p.control(SIO_SET_BAUDRATE, 0x4138, 0)?;
        p.control(SIO_SET_DATA, 0x0008, PORT_A)?; // 8 data bits, no parity, 1 stop
        p.control(SIO_SET_FLOW_CTRL, 0, PORT_A)?;
        p.control(SIO_SET_LATENCY, 2, PORT_A)?;
        p.control(SIO_RESET, PURGE_TX, PORT_A)?;
        p.purge_rx()?;
        Ok(p)
    }

    pub fn product(&self) -> Option<String> {
        self.product.clone()
    }

    fn control(&self, request: u8, value: u16, index: u16) -> Result<()> {
        let req = ControlOut {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request,
            value,
            index,
            data: &[],
        };
        self.interface.control_out(req, CONTROL_TIMEOUT).wait()?;
        Ok(())
    }

    /// Wait up to `timeout` for one IN transfer and strip the status header
    /// from every packet in it. A timeout leaves the transfer pending.
    fn fill(&mut self, timeout: Duration) -> Result<()> {
        if self.ep_in.pending() == 0 {
            self.ep_in.submit(Buffer::new(IN_TRANSFER));
        }
        let Some(c) = self.ep_in.wait_next_complete(timeout) else {
            return Ok(());
        };
        c.status?;
        for pkt in c.buffer.chunks(PACKET) {
            self.rx.extend(pkt.iter().skip(2));
        }
        Ok(())
    }
}

impl Port for FtdiPort {
    fn send(&mut self, data: &[u8]) -> Result<()> {
        self.ep_out
            .transfer_blocking(data.into(), WRITE_TIMEOUT)
            .status?;
        Ok(())
    }

    fn recv(&mut self, n: usize, timeout: Duration) -> Result<Vec<u8>> {
        let deadline = Instant::now() + timeout;
        while self.rx.len() < n {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            self.fill(left.min(Duration::from_millis(50)))?;
        }
        let n = n.min(self.rx.len());
        Ok(self.rx.drain(..n).collect())
    }

    fn purge_rx(&mut self) -> Result<()> {
        self.control(SIO_RESET, PURGE_RX, PORT_A)?;
        // Drop anything already received into a completed transfer.
        self.fill(Duration::from_millis(5))?;
        self.rx.clear();
        Ok(())
    }
}
