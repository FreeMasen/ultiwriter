//! Byte-level encoding of box commands and replies.
//!
//! Every reply is the command's mnemonic echoed back, then a payload, then
//! a dialect-specific trailer (see `protocol-spec/PROTOCOL.md`).

/// Bytes per write page. Short final pages are padded with `0xFF`.
pub const PAGE: usize = 128;
/// Largest read the box serves in one request.
pub const MAX_READ: usize = 4096;

/// Firmware dialect, chosen by the handshake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    /// ASCII mnemonics (`rd1`, `wr1`, ...); "Reader/Writer Box" firmware.
    Ascii,
    /// Two-digit opcodes (`13`, `14`, ...); older firmware.
    Numeric,
}

impl Dialect {
    pub fn terminator(self) -> &'static [u8] {
        match self {
            Dialect::Ascii => b"\r\n",
            Dialect::Numeric => b"\n",
        }
    }

    pub fn trailer(self) -> &'static [u8] {
        match self {
            Dialect::Ascii => b"\r\n>",
            Dialect::Numeric => b"\n",
        }
    }
}

/// Card family, as the Windows app's `kind` argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    /// `kind == 1`: Husqvarna Viking `.HUS` card, JEDEC NOR flash.
    Viking,
    /// `kind == 4`: Vikant multi-format card, opcodes `29`/`28` only.
    Vikant,
}

impl CardKind {
    pub fn is_native(self) -> bool {
        self == CardKind::Viking
    }
}

/// Fixed-form commands: mnemonic + terminator, reply = mnemonic + payload + trailer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    LedOn,
    LedOff,
    CardDetect,
    Identify,
    Erase,
}

impl Op {
    pub fn mnemonic(self, d: Dialect) -> &'static [u8] {
        use Dialect::*;
        match (self, d) {
            (Op::LedOn, Ascii) => b"ld1",
            (Op::LedOn, Numeric) => b"31",
            (Op::LedOff, Ascii) => b"ld0",
            (Op::LedOff, Numeric) => b"32",
            (Op::CardDetect, Ascii) => b"cd1",
            (Op::CardDetect, Numeric) => b"10",
            (Op::Identify, Ascii) => b"id1",
            (Op::Identify, Numeric) => b"11",
            (Op::Erase, Ascii) => b"er1",
            (Op::Erase, Numeric) => b"12",
        }
    }

    fn payload_len(self) -> usize {
        match self {
            Op::LedOn | Op::LedOff => 0,
            Op::CardDetect | Op::Erase => 1,
            Op::Identify => 2,
        }
    }

    pub fn request(self, d: Dialect) -> Vec<u8> {
        [self.mnemonic(d), d.terminator()].concat()
    }

    pub fn reply_len(self, d: Dialect) -> usize {
        self.mnemonic(d).len() + self.payload_len() + d.trailer().len()
    }

    /// Payload bytes of a well-formed reply, or `None` if the echo is wrong.
    pub fn payload(self, d: Dialect, reply: &[u8]) -> Option<&[u8]> {
        let m = self.mnemonic(d);
        let body = reply.strip_prefix(m)?;
        (body.len() >= self.payload_len()).then(|| &body[..self.payload_len()])
    }
}

/// Handshake request and the exact reply that selects the dialect.
pub fn handshake(d: Dialect) -> (&'static [u8], &'static [u8]) {
    match d {
        Dialect::Ascii => (b"vr1\r\n", b"v10\r\n>"),
        Dialect::Numeric => (b"30\n", b"10\n"),
    }
}

/// Status byte meaning success in erase/write acks.
pub const STATUS_OK: u8 = b'1';
pub const STATUS_FAIL: u8 = b'0';

/// Layout of a chunked read for one dialect/kind combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadFrame {
    pub mnemonic: &'static [u8],
    pub terminator: &'static [u8],
    pub trailer: &'static [u8],
}

impl ReadFrame {
    pub fn new(d: Dialect, kind: CardKind) -> Self {
        match (kind.is_native(), d) {
            (true, Dialect::Ascii) => Self {
                mnemonic: b"rd1",
                terminator: b"\r\n",
                trailer: b"\r\n>",
            },
            (true, Dialect::Numeric) => Self {
                mnemonic: b"13",
                terminator: b"\n",
                trailer: b"\n",
            },
            // The app sends `29` with a bare LF regardless of dialect.
            (false, _) => Self {
                mnemonic: b"29",
                terminator: b"\n",
                trailer: b"\n",
            },
        }
    }

    fn header(&self, addr: u32, len: u16) -> Vec<u8> {
        let mut h = self.mnemonic.to_vec();
        h.extend_from_slice(&addr.to_le_bytes());
        h.extend_from_slice(&len.to_le_bytes());
        h
    }

    pub fn request(&self, addr: u32, len: u16) -> Vec<u8> {
        [self.header(addr, len).as_slice(), self.terminator].concat()
    }

    pub fn reply_len(&self, len: u16) -> usize {
        self.mnemonic.len() + 6 + len as usize + self.trailer.len()
    }

    /// Validate the echoed header/trailer and return the data bytes.
    pub fn data<'a>(&self, addr: u32, len: u16, reply: &'a [u8]) -> Option<&'a [u8]> {
        let rest = reply.strip_prefix(self.header(addr, len).as_slice())?;
        let data = rest.strip_suffix(self.trailer)?;
        (data.len() == len as usize).then_some(data)
    }
}

/// Layout of a 128-byte page write for one dialect/kind combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WriteFrame {
    pub mnemonic: &'static [u8],
    pub terminator: &'static [u8],
    /// Index of the status byte in the ack, or `None` when the ack has none.
    pub status_at: Option<usize>,
    pub ack_len: usize,
}

impl WriteFrame {
    pub fn new(d: Dialect, kind: CardKind) -> Self {
        // The write terminator depends only on the dialect.
        let terminator = d.terminator();
        match (kind.is_native(), d) {
            (true, Dialect::Ascii) => Self {
                mnemonic: b"wr1",
                terminator,
                status_at: Some(3),
                ack_len: 7,
            },
            (true, Dialect::Numeric) => Self {
                mnemonic: b"14",
                terminator,
                status_at: Some(2),
                ack_len: 4,
            },
            (false, _) => Self {
                mnemonic: b"28",
                terminator,
                status_at: None,
                ack_len: 3,
            },
        }
    }

    /// Encode one page; `data` shorter than [`PAGE`] is padded with `0xFF`.
    pub fn request(&self, addr: u32, data: &[u8]) -> Vec<u8> {
        assert!(data.len() <= PAGE, "page larger than {PAGE} bytes");
        let mut r = self.mnemonic.to_vec();
        r.extend_from_slice(&addr.to_le_bytes());
        r.extend_from_slice(data);
        r.resize(r.len() + PAGE - data.len(), 0xFF);
        r.extend_from_slice(self.terminator);
        r
    }

    /// `Some(ok)` once the ack is complete, `None` while more bytes are needed.
    pub fn ack(&self, reply: &[u8]) -> Option<bool> {
        if reply.len() < self.ack_len {
            return None;
        }
        Some(match self.status_at {
            Some(i) => reply.starts_with(self.mnemonic) && reply[i] == STATUS_OK,
            None => reply.starts_with(b"28\n"),
        })
    }
}

/// JEDEC manufacturer/device ID returned by `id1`/`11`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlashId {
    pub manufacturer: u8,
    pub device: u8,
}

/// Size the app assumes when the ID is not in its table.
pub const DEFAULT_SIZE: u32 = 0x8_0000;

impl FlashId {
    /// Card size in bytes, per the app's table at `0x459058`.
    pub fn size(self) -> u32 {
        const K512: u32 = 0x8_0000;
        const M1: u32 = 0x10_0000;
        let (small, large): (&[u8], &[u8]) = match self.manufacturer {
            0x01 | 0x04 | 0xAD | 0xC2 => (&[0xA4, 0x23, 0xAB], &[0xD5, 0xD6, 0x58]),
            0x1F => (&[0x13], &[0x22, 0x21]),
            0x20 => (&[0xE2, 0xD5, 0xD6], &[0xF1, 0xEC, 0x58]),
            _ => return DEFAULT_SIZE,
        };
        if small.contains(&self.device) {
            K512
        } else if large.contains(&self.device) {
            M1
        } else {
            DEFAULT_SIZE
        }
    }

    pub fn manufacturer_name(self) -> &'static str {
        match self.manufacturer {
            0x01 => "AMD",
            0x04 => "Fujitsu",
            0x1F => "Atmel",
            0x20 => "ST",
            0xAD => "Hynix",
            0xC2 => "Macronix",
            _ => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_request_matches_hardware_capture() {
        let f = ReadFrame::new(Dialect::Ascii, CardKind::Viking);
        assert_eq!(f.request(0, 256), b"rd1\x00\x00\x00\x00\x00\x01\r\n");
        assert_eq!(f.reply_len(256), 268);
    }

    #[test]
    fn read_reply_validates_echo_and_trailer() {
        let f = ReadFrame::new(Dialect::Ascii, CardKind::Viking);
        let mut reply = b"rd1\x10\x00\x00\x00\x02\x00".to_vec();
        reply.extend_from_slice(b"\xAB\xCD\r\n>");
        assert_eq!(f.data(0x10, 2, &reply), Some(&b"\xAB\xCD"[..]));
        assert_eq!(f.data(0x11, 2, &reply), None);
        assert_eq!(f.data(0x10, 2, &reply[..reply.len() - 1]), None);
    }

    #[test]
    fn numeric_and_vikant_read_frames() {
        assert_eq!(
            ReadFrame::new(Dialect::Numeric, CardKind::Viking).request(1, 2),
            b"13\x01\x00\x00\x00\x02\x00\n"
        );
        assert_eq!(
            ReadFrame::new(Dialect::Ascii, CardKind::Vikant).request(1, 2),
            b"29\x01\x00\x00\x00\x02\x00\n"
        );
    }

    #[test]
    fn write_frame_lengths_and_padding() {
        let a = WriteFrame::new(Dialect::Ascii, CardKind::Viking).request(0x80, &[1, 2, 3]);
        assert_eq!(a.len(), 137);
        assert_eq!(&a[..7], b"wr1\x80\x00\x00\x00");
        assert_eq!(&a[7..10], &[1, 2, 3]);
        assert!(a[10..135].iter().all(|&b| b == 0xFF));
        assert_eq!(&a[135..], b"\r\n");
        assert_eq!(
            WriteFrame::new(Dialect::Numeric, CardKind::Viking)
                .request(0, &[])
                .len(),
            135
        );
        assert_eq!(
            WriteFrame::new(Dialect::Numeric, CardKind::Vikant)
                .request(0, &[])
                .len(),
            135
        );
    }

    #[test]
    fn write_acks() {
        let a = WriteFrame::new(Dialect::Ascii, CardKind::Viking);
        assert_eq!(a.ack(b"wr11\r\n"), None);
        assert_eq!(a.ack(b"wr11\r\n>"), Some(true));
        assert_eq!(a.ack(b"wr10\r\n>"), Some(false));
        let n = WriteFrame::new(Dialect::Numeric, CardKind::Viking);
        assert_eq!(n.ack(b"141\n"), Some(true));
        assert_eq!(
            WriteFrame::new(Dialect::Numeric, CardKind::Vikant).ack(b"28\n"),
            Some(true)
        );
    }

    #[test]
    fn op_reply_lengths_match_app() {
        use Dialect::*;
        let cases = [
            (Op::LedOn, Ascii, 6),
            (Op::LedOff, Numeric, 3),
            (Op::CardDetect, Ascii, 7),
            (Op::CardDetect, Numeric, 4),
            (Op::Identify, Ascii, 8),
            (Op::Identify, Numeric, 5),
            (Op::Erase, Ascii, 7),
            (Op::Erase, Numeric, 4),
        ];
        for (op, d, n) in cases {
            assert_eq!(op.reply_len(d), n, "{op:?} {d:?}");
        }
        assert_eq!(
            Op::Identify.payload(Ascii, b"id1\x01\xd5\r\n>"),
            Some(&b"\x01\xd5"[..])
        );
        assert_eq!(Op::CardDetect.payload(Ascii, b"cmder\r\n>"), None);
    }

    #[test]
    fn flash_sizes() {
        let id = |m, d| {
            FlashId {
                manufacturer: m,
                device: d,
            }
            .size()
        };
        assert_eq!(id(0x01, 0xD5), 0x10_0000); // the test card: Am29F080
        assert_eq!(id(0x01, 0xA4), 0x8_0000);
        assert_eq!(id(0x20, 0xD5), 0x8_0000); // ST maps D5 to 512 KiB
        assert_eq!(id(0x1F, 0x21), 0x10_0000);
        assert_eq!(id(0x99, 0xD5), DEFAULT_SIZE);
    }
}
