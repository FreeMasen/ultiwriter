//! High-level card operations over any byte [`Port`].

use std::fmt;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::frame::{
    self, CardKind, Dialect, FlashId, MAX_READ, Op, PAGE, ReadFrame, STATUS_FAIL, STATUS_OK,
    WriteFrame,
};

/// A bidirectional byte channel to the box.
pub trait Port {
    fn send(&mut self, data: &[u8]) -> Result<()>;
    /// Read until `n` bytes have arrived or `timeout` expires; may return fewer.
    fn recv(&mut self, n: usize, timeout: Duration) -> Result<Vec<u8>>;
    /// Discard anything already received.
    fn purge_rx(&mut self) -> Result<()>;
}

/// Lets a caller lend a port to a [`Client`] and inspect it afterwards.
impl<T: Port + ?Sized> Port for &mut T {
    fn send(&mut self, data: &[u8]) -> Result<()> {
        (**self).send(data)
    }
    fn recv(&mut self, n: usize, timeout: Duration) -> Result<Vec<u8>> {
        (**self).recv(n, timeout)
    }
    fn purge_rx(&mut self) -> Result<()> {
        (**self).purge_rx()
    }
}

#[derive(Debug)]
pub enum Error {
    NoDevice,
    Usb(String),
    Handshake,
    NoCard,
    /// The box answered, but not with what the command expects.
    BadReply {
        cmd: String,
        got: Vec<u8>,
    },
    /// The box reported failure (`'0'` status).
    Failed(&'static str),
    Timeout(&'static str),
    Unsupported(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoDevice => write!(f, "no Vikant box found on USB"),
            Error::Usb(e) => write!(f, "USB error: {e}"),
            Error::Handshake => write!(f, "box did not answer the handshake"),
            Error::NoCard => write!(f, "no card in the slot"),
            Error::BadReply { cmd, got } => write!(
                f,
                "unexpected reply to {cmd}: {:?}",
                got.escape_ascii().to_string()
            ),
            Error::Failed(op) => write!(f, "box reported failure: {op}"),
            Error::Timeout(op) => write!(f, "timed out waiting for {op}"),
            Error::Unsupported(op) => write!(f, "not supported: {op}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Per-reply timeout, matching the app's ~1 s polling budget.
const REPLY_TIMEOUT: Duration = Duration::from_millis(1000);
const PAGE_TIMEOUT: Duration = Duration::from_secs(5);
/// Chip erase on 29F0x0 parts takes several seconds; datasheets allow ~30 s.
const ERASE_TIMEOUT: Duration = Duration::from_secs(90);

pub struct Client<P: Port> {
    port: P,
    dialect: Dialect,
}

impl<P: Port> Client<P> {
    /// Handshake and select the dialect. Tries ASCII first: the first command
    /// after the port opens is often dropped, so each probe is retried.
    pub fn connect(mut port: P) -> Result<Self> {
        for d in [
            Dialect::Ascii,
            Dialect::Ascii,
            Dialect::Ascii,
            Dialect::Numeric,
            Dialect::Numeric,
        ] {
            let (req, want) = frame::handshake(d);
            port.purge_rx()?;
            port.send(req)?;
            sleep(Duration::from_millis(250));
            if port.recv(want.len(), REPLY_TIMEOUT)? == want {
                let mut c = Client { port, dialect: d };
                c.simple(Op::LedOff)?;
                return Ok(c);
            }
        }
        Err(Error::Handshake)
    }

    pub fn dialect(&self) -> Dialect {
        self.dialect
    }

    pub fn into_port(self) -> P {
        self.port
    }

    fn exchange(&mut self, req: &[u8], n: usize, timeout: Duration) -> Result<Vec<u8>> {
        self.port.purge_rx()?;
        self.port.send(req)?;
        self.port.recv(n, timeout)
    }

    fn simple(&mut self, op: Op) -> Result<Vec<u8>> {
        let d = self.dialect;
        self.exchange(&op.request(d), op.reply_len(d), REPLY_TIMEOUT)
    }

    fn bad(op: Op, d: Dialect, got: Vec<u8>) -> Error {
        Error::BadReply {
            cmd: op.mnemonic(d).escape_ascii().to_string(),
            got,
        }
    }

    pub fn set_led(&mut self, on: bool) -> Result<()> {
        self.simple(if on { Op::LedOn } else { Op::LedOff })
            .map(drop)
    }

    /// Run `f` with the activity LED on, as the app does around every operation.
    fn with_led<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.set_led(true)?;
        let r = f(self);
        let off = self.set_led(false);
        let v = r?;
        off.map(|_| v)
    }

    pub fn card_present(&mut self) -> Result<bool> {
        let d = self.dialect;
        let r = self.with_led(|c| c.simple(Op::CardDetect))?;
        match Op::CardDetect.payload(d, &r) {
            Some([s]) => Ok(*s == STATUS_OK),
            _ => Err(Self::bad(Op::CardDetect, d, r)),
        }
    }

    /// JEDEC ID of a Viking (native flash) card.
    pub fn identify(&mut self) -> Result<FlashId> {
        let d = self.dialect;
        let r = self.with_led(|c| c.simple(Op::Identify))?;
        match Op::Identify.payload(d, &r) {
            Some(&[manufacturer, device]) => Ok(FlashId {
                manufacturer,
                device,
            }),
            _ => Err(Self::bad(Op::Identify, d, r)),
        }
    }

    /// Read `buf.len()` bytes starting at `addr`, in chunks of up to 4 KiB.
    pub fn read(&mut self, kind: CardKind, addr: u32, buf: &mut [u8]) -> Result<()> {
        self.read_with_progress(kind, addr, buf, |_| {})
    }

    pub fn read_with_progress(
        &mut self,
        kind: CardKind,
        addr: u32,
        buf: &mut [u8],
        mut progress: impl FnMut(usize),
    ) -> Result<()> {
        let f = ReadFrame::new(self.dialect, kind);
        let total = buf.len();
        self.with_led(|c| {
            for (i, chunk) in buf.chunks_mut(MAX_READ).enumerate() {
                let a = addr + (i * MAX_READ) as u32;
                let len = chunk.len() as u16;
                let mut last = Vec::new();
                // Retry a garbled chunk; reads are side-effect free.
                for _ in 0..3 {
                    last = c.exchange(&f.request(a, len), f.reply_len(len), REPLY_TIMEOUT * 2)?;
                    if let Some(data) = f.data(a, len, &last) {
                        chunk.copy_from_slice(data);
                        last.clear();
                        break;
                    }
                }
                if !last.is_empty() {
                    return Err(Error::BadReply {
                        cmd: format!("read @{a:#x}"),
                        got: last,
                    });
                }
                progress((i * MAX_READ + chunk.len()).min(total));
            }
            Ok(())
        })
    }

    /// Program `data` at `addr` in 128-byte pages (short tail padded with
    /// `0xFF`). Flash can only clear bits: erase first unless the target is blank.
    pub fn write(&mut self, kind: CardKind, addr: u32, data: &[u8]) -> Result<()> {
        self.write_pages(kind, addr, data, false, |_| {})
    }

    /// Like [`write`](Self::write) but skips pages that are entirely `0xFF`,
    /// which is a no-op on erased flash.
    pub fn write_pages(
        &mut self,
        kind: CardKind,
        addr: u32,
        data: &[u8],
        skip_blank: bool,
        mut progress: impl FnMut(usize),
    ) -> Result<()> {
        let f = WriteFrame::new(self.dialect, kind);
        self.with_led(|c| {
            for (i, page) in data.chunks(PAGE).enumerate() {
                if !(skip_blank && page.iter().all(|&b| b == 0xFF)) {
                    c.write_page(&f, addr + (i * PAGE) as u32, page)?;
                }
                progress((i * PAGE + page.len()).min(data.len()));
            }
            Ok(())
        })
    }

    fn write_page(&mut self, f: &WriteFrame, addr: u32, page: &[u8]) -> Result<()> {
        self.port.purge_rx()?;
        self.port.send(&f.request(addr, page))?;
        let deadline = Instant::now() + PAGE_TIMEOUT;
        let mut ack = Vec::new();
        while Instant::now() < deadline {
            ack.extend(self.port.recv(f.ack_len - ack.len(), REPLY_TIMEOUT)?);
            match f.ack(&ack) {
                Some(true) => return Ok(()),
                Some(false) if f.status_at.is_some_and(|i| ack[i] == STATUS_FAIL) => {
                    return Err(Error::Failed("page write"));
                }
                Some(false) => {
                    return Err(Error::BadReply {
                        cmd: format!("write @{addr:#x}"),
                        got: ack,
                    });
                }
                None => {}
            }
        }
        Err(Error::Timeout("page write ack"))
    }

    /// Chip-erase a Viking (native flash) card.
    pub fn erase(&mut self) -> Result<()> {
        let d = self.dialect;
        let op = Op::Erase;
        self.with_led(|c| {
            c.port.purge_rx()?;
            c.port.send(&op.request(d))?;
            let deadline = Instant::now() + ERASE_TIMEOUT;
            let mut r = Vec::new();
            while Instant::now() < deadline {
                r.extend(c.port.recv(op.reply_len(d) - r.len(), REPLY_TIMEOUT)?);
                if r.len() < op.reply_len(d) {
                    continue;
                }
                return match op.payload(d, &r) {
                    Some([STATUS_OK]) => Ok(()),
                    Some([STATUS_FAIL]) => Err(Error::Failed("erase")),
                    _ => Err(Self::bad(op, d, r)),
                };
            }
            Err(Error::Timeout("erase"))
        })
    }
}
