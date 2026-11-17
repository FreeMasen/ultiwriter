//! Husqvarna Viking card image (`kind == 1`), as built by `0x4928c0` and
//! read by `0x492990` in `UltimateBox.exe`. See `protocol-spec/CARD.md`.
//!
//! Multi-byte values are big-endian except the leading magic.

use std::fmt;

/// LE32 at offset 0.
pub const MAGIC: u32 = 0x80C8;
/// Directory marker at `5 + card[4]`.
pub const DIR_MARK: u8 = 0x23;
/// Bytes 1..3 of every design record.
pub const DESIGN_MAGIC: [u8; 2] = [0xC4, 0x40];
/// The app builds images in a 512 KiB buffer, even for 1 MiB cards.
pub const IMAGE_CAPACITY: usize = 0x8_0000;
/// Directory slots per bank, including the separator in multi-bank form.
const BANK: usize = 35;
const BANK_SEP: u8 = 3;
/// Slots: 35 in single-bank form; 3 banks of 34 + separator otherwise.
pub const MAX_DESIGNS_SMALL: usize = 35;
pub const MAX_DESIGNS: usize = 102;
const NAME_LEN: usize = 16;
const HEADER_LEN: usize = 29;
const BLOCK_LEN: usize = 7;
const TRAILER: [u8; 11] = [0; 11];
const LONG_START: [u8; 2] = [0x80, 0x01];
const LONG_END: [u8; 2] = [0x80, 0x02];

/// Thread-colour codes, indexed by the app's 38-colour palette (`0x491a1c`).
pub const COLOR_CODES: [u8; 38] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x21, 0x22, 0x23,
    0x24, 0x25, 0x26, 0x29, 0x2A, 0x2B,
];

#[derive(Debug, PartialEq, Eq)]
pub enum CardError {
    /// Not a Viking card image (missing `#` directory marker).
    NotViking,
    /// The image ends early; at least `need` bytes are required.
    Truncated {
        need: usize,
    },
    Malformed(String),
    TooManyDesigns(usize),
    TooLarge(usize),
}

impl fmt::Display for CardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CardError::NotViking => write!(f, "not a Husqvarna Viking card image"),
            CardError::Truncated { need } => write!(f, "card image truncated (need {need} bytes)"),
            CardError::Malformed(m) => write!(f, "malformed card image: {m}"),
            CardError::TooManyDesigns(n) => {
                write!(f, "{n} designs; a card holds at most {MAX_DESIGNS}")
            }
            CardError::TooLarge(n) => write!(f, "image is {n} bytes; limit is {IMAGE_CAPACITY}"),
        }
    }
}

impl std::error::Error for CardError {}

/// One relative move in card units (0.1 mm), y down. The app works in y up
/// and negates y as it writes (`prev_y - y`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stitch {
    pub dx: i16,
    pub dy: i16,
}

impl Stitch {
    /// Deltas beyond ±127 need the 8-byte long form.
    pub fn is_long(self) -> bool {
        !(-127..=127).contains(&self.dx) || !(-127..=127).contains(&self.dy)
    }

    /// Size in the 2-byte units the header counts.
    fn units(self) -> usize {
        if self.is_long() { 4 } else { 1 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorBlock {
    /// Thread colour code (see [`COLOR_CODES`]).
    pub color: u8,
    /// Absolute position where the block starts, card coordinates.
    pub start: (i16, i16),
    pub stitches: Vec<Stitch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Design {
    /// 16-byte space-padded name; the app writes the 2-digit slot number.
    pub name: [u8; NAME_LEN],
    /// (min x, min y, max x, max y), absolute, card coordinates.
    pub bbox: (i16, i16, i16, i16),
    pub blocks: Vec<ColorBlock>,
}

impl Design {
    pub fn name_str(&self) -> String {
        String::from_utf8_lossy(&self.name).trim_end().to_string()
    }

    /// Absolute stitch positions, starting from the origin.
    pub fn positions(&self) -> Vec<(i32, i32)> {
        let (mut x, mut y) = (0i32, 0i32);
        let mut out = Vec::new();
        for s in self.blocks.iter().flat_map(|b| &b.stitches) {
            x += s.dx as i32;
            y += s.dy as i32;
            out.push((x, y));
        }
        out
    }

    pub fn stitch_count(&self) -> usize {
        self.blocks.iter().map(|b| b.stitches.len()).sum()
    }

    fn units(&self) -> usize {
        self.blocks
            .iter()
            .flat_map(|b| &b.stitches)
            .map(|s| s.units())
            .sum()
    }

    /// Record size in bytes: header + colour table + stitch bytes.
    pub fn encoded_len(&self) -> usize {
        HEADER_LEN + BLOCK_LEN * self.blocks.len() + 2 * self.units()
    }

    fn parse(d: &[u8], p: usize) -> Result<(Design, usize), CardError> {
        let need = |n: usize| {
            if d.len() < n {
                Err(CardError::Truncated { need: n })
            } else {
                Ok(())
            }
        };
        need(p + HEADER_LEN)?;
        let be = |o: usize| i16::from_be_bytes([d[o], d[o + 1]]);
        let nblk = d[p] as usize;
        if d[p + 1..p + 3] != DESIGN_MAGIC {
            return Err(CardError::Malformed(format!(
                "design at {p:#x} lacks c4 40 magic"
            )));
        }
        let mut name = [0; NAME_LEN];
        name.copy_from_slice(&d[p + 3..p + 3 + NAME_LEN]);
        // Stored as app-space (max x, -min y, min x, -max y) = card-space (max x, max y, min x, min y).
        let (max_x, max_y, min_x, min_y) = (be(p + 19), be(p + 21), be(p + 23), be(p + 25));
        let units = u16::from_be_bytes([d[p + 27], d[p + 28]]) as usize;
        let table = p + HEADER_LEN;
        let stitches_at = table + BLOCK_LEN * nblk;
        let end = stitches_at + 2 * units;
        need(end)?;

        let mut blocks = Vec::with_capacity(nblk);
        let mut i = stitches_at;
        for b in 0..nblk {
            let o = table + BLOCK_LEN * b;
            let block_units = u16::from_be_bytes([d[o], d[o + 1]]) as usize;
            let mut stitches = Vec::new();
            let mut used = 0;
            while used < block_units {
                if i + 2 > end {
                    return Err(CardError::Malformed(format!(
                        "block {b} of design at {p:#x} overruns"
                    )));
                }
                if d[i..i + 2] == LONG_START {
                    if i + 8 > end || d[i + 6..i + 8] != LONG_END {
                        return Err(CardError::Malformed(format!("bad long stitch at {i:#x}")));
                    }
                    stitches.push(Stitch {
                        dx: be(i + 2),
                        dy: be(i + 4),
                    });
                    i += 8;
                    used += 4;
                } else {
                    stitches.push(Stitch {
                        dx: d[i] as i8 as i16,
                        dy: d[i + 1] as i8 as i16,
                    });
                    i += 2;
                    used += 1;
                }
            }
            blocks.push(ColorBlock {
                color: d[o + 2],
                start: (be(o + 3), be(o + 5)),
                stitches,
            });
        }
        if i != end {
            return Err(CardError::Malformed(format!(
                "design at {p:#x}: block counts disagree with length"
            )));
        }
        Ok((
            Design {
                name,
                bbox: (min_x, min_y, max_x, max_y),
                blocks,
            },
            end,
        ))
    }

    fn encode(&self, out: &mut Vec<u8>) {
        let (min_x, min_y, max_x, max_y) = self.bbox;
        out.push(self.blocks.len() as u8);
        out.extend_from_slice(&DESIGN_MAGIC);
        out.extend_from_slice(&self.name);
        for v in [max_x, max_y, min_x, min_y] {
            out.extend_from_slice(&v.to_be_bytes());
        }
        out.extend_from_slice(&(self.units() as u16).to_be_bytes());
        for b in &self.blocks {
            let units: usize = b.stitches.iter().map(|s| s.units()).sum();
            out.extend_from_slice(&(units as u16).to_be_bytes());
            out.push(b.color);
            out.extend_from_slice(&b.start.0.to_be_bytes());
            out.extend_from_slice(&b.start.1.to_be_bytes());
        }
        for s in self.blocks.iter().flat_map(|b| &b.stitches) {
            if s.is_long() {
                out.extend_from_slice(&LONG_START);
                out.extend_from_slice(&s.dx.to_be_bytes());
                out.extend_from_slice(&s.dy.to_be_bytes());
                out.extend_from_slice(&LONG_END);
            } else {
                out.extend_from_slice(&[s.dx as i8 as u8, s.dy as i8 as u8]);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Card {
    pub designs: Vec<Design>,
}

/// Directory geometry: bitmap length and offset-table length.
fn layout(n: usize) -> (usize, usize) {
    if n > MAX_DESIGNS_SMALL {
        (3 * BANK, 3 * 0x44 + 2)
    } else {
        (BANK, 0x48)
    }
}

impl Card {
    /// Parse a card image (it may be followed by anything, e.g. `0xFF`).
    pub fn parse(d: &[u8]) -> Result<Card, CardError> {
        Self::parse_len(d).map(|(c, _)| c)
    }

    /// Parse, also returning the image length including the zero trailer.
    pub fn parse_len(d: &[u8]) -> Result<(Card, usize), CardError> {
        let need = |n: usize| {
            if d.len() < n {
                Err(CardError::Truncated { need: n })
            } else {
                Ok(())
            }
        };
        need(6)?;
        let base = d[4] as usize;
        need(6 + base)?;
        if d[5 + base] != DIR_MARK {
            return Err(CardError::NotViking);
        }
        // Mirror the reader: another bank follows while the last byte of
        // the current one is a separator (3, or 2 in an older variant).
        let mut p = 6 + base;
        let mut banks = 1;
        let mut table = 0x48;
        loop {
            need(p + BANK)?;
            let bank = &d[p..p + BANK];
            let sep = bank[0x22];
            if !matches!(sep, 2 | 3) || bank[0x20] > 1 || bank[0x21] > 1 {
                break;
            }
            table = if sep == 3 {
                banks * 0x44 + 2
            } else {
                banks * 0x60
            };
            p += BANK;
            banks += 1;
        }
        p += BANK + table;
        if banks > 1 {
            p -= BANK;
        }

        let mut designs = Vec::new();
        loop {
            need(p + 3)?;
            let head = &d[p..p + 3];
            if head.iter().all(|&b| b == 0) || head.iter().all(|&b| b == 0xFF) {
                break;
            }
            let (design, end) = Design::parse(d, p)?;
            designs.push(design);
            p = end;
        }
        let len = if d[p..].len() >= TRAILER.len() && d[p..p + TRAILER.len()] == TRAILER {
            p + TRAILER.len()
        } else {
            p
        };
        Ok((Card { designs }, len))
    }

    /// Encode the image the way the app's builder does. Bytes after the
    /// returned length are `0xFF` (erased flash).
    pub fn to_image(&self) -> Result<Vec<u8>, CardError> {
        let n = self.designs.len();
        if n > MAX_DESIGNS {
            return Err(CardError::TooManyDesigns(n));
        }
        let (bitmap_len, table_len) = layout(n);
        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC.to_le_bytes());
        out.extend_from_slice(&[0, DIR_MARK]);
        let mut bitmap = vec![0u8; bitmap_len];
        if bitmap_len == BANK {
            bitmap[..n].fill(1);
        } else {
            for sep in [0x22, 0x45, 0x68] {
                bitmap[sep] = BANK_SEP;
            }
            // Design k occupies slot k, skipping the separators.
            for k in 0..n {
                bitmap[k + (k >= 34) as usize + (k >= 68) as usize] = 1;
            }
        }
        out.extend_from_slice(&bitmap);
        // Entry 0 is the table length (offset of design 0 from the table);
        // entry i+1 is the size of design i, except after the last.
        let mut table = vec![0u8; table_len];
        table[..2].copy_from_slice(&(table_len as u16).to_be_bytes());
        for (i, d) in self.designs.iter().enumerate().take(n.saturating_sub(1)) {
            table[2 * (i + 1)..2 * (i + 2)]
                .copy_from_slice(&(d.encoded_len() as u16).to_be_bytes());
        }
        out.extend_from_slice(&table);
        for d in &self.designs {
            d.encode(&mut out);
        }
        out.extend_from_slice(&TRAILER);
        if out.len() > IMAGE_CAPACITY {
            return Err(CardError::TooLarge(out.len()));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> [u8; NAME_LEN] {
        let mut n = [b' '; NAME_LEN];
        n[..s.len()].copy_from_slice(s.as_bytes());
        n
    }

    fn design(slot: usize, moves: &[(i16, i16)]) -> Design {
        let stitches: Vec<Stitch> = moves.iter().map(|&(dx, dy)| Stitch { dx, dy }).collect();
        let half = stitches.len() / 2;
        Design {
            name: name(&format!("{slot:02}")),
            bbox: (-10, -20, 30, 40),
            blocks: vec![
                ColorBlock {
                    color: 0x05,
                    start: (0, 0),
                    stitches: stitches[..half].to_vec(),
                },
                ColorBlock {
                    color: 0x21,
                    start: (7, -3),
                    stitches: stitches[half..].to_vec(),
                },
            ],
        }
    }

    #[test]
    fn roundtrip_with_long_stitches_and_two_designs() {
        let card = Card {
            designs: vec![
                design(1, &[(1, 2), (-127, 127), (300, -5), (0, 0)]),
                design(2, &[(5, 5), (-400, 128)]),
            ],
        };
        let img = card.to_image().unwrap();
        assert_eq!(&img[..6], &[0xC8, 0x80, 0, 0, 0, 0x23]);
        assert_eq!(&img[6..9], &[1, 1, 0]);
        // Table: 0x48, then size of design 0.
        assert_eq!(&img[0x29..0x2b], &[0x00, 0x48]);
        let d0 = card.designs[0].encoded_len() as u16;
        assert_eq!(&img[0x2b..0x2d], &d0.to_be_bytes());
        assert_eq!(&img[0x71..0x74], &[2, 0xC4, 0x40]);
        let mut padded = img.clone();
        padded.resize(4096, 0xFF);
        let (back, len) = Card::parse_len(&padded).unwrap();
        assert_eq!(back, card);
        assert_eq!(len, img.len());
    }

    #[test]
    fn multi_bank_directory() {
        let card = Card {
            designs: (1..=40).map(|i| design(i, &[(1, 1), (2, 2)])).collect(),
        };
        let img = card.to_image().unwrap();
        assert_eq!(img[6 + 0x22], 3);
        assert_eq!(img[6 + 0x23], 1); // design 34 sits after the first separator
        assert_eq!(&img[0x6f..0x71], &[0x00, 0xCE]);
        assert_eq!(&img[0x13d..0x140], &[2, 0xC4, 0x40]);
        assert_eq!(Card::parse(&img).unwrap(), card);
    }

    #[test]
    fn rejects_non_viking_and_reports_truncation() {
        assert_eq!(Card::parse(&[0xFF; 512]), Err(CardError::NotViking)); // erased card
        let img = Card {
            designs: vec![design(1, &[(1, 1); 50])],
        }
        .to_image()
        .unwrap();
        assert!(matches!(
            Card::parse(&img[..0x80]),
            Err(CardError::Truncated { .. })
        ));
    }

    /// The real card dumped from hardware, if present (it is not in git).
    #[test]
    fn real_card_dump_roundtrips() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../dumps/card1.bin");
        let Ok(dump) = std::fs::read(path) else {
            eprintln!("skipping: {path} not present");
            return;
        };
        let (card, len) = Card::parse_len(&dump).unwrap();
        assert_eq!(len, 0x1adc);
        let d = &card.designs[0];
        assert_eq!(
            (card.designs.len(), d.name_str().as_str(), d.stitch_count()),
            (1, "01", 3358)
        );
        assert_eq!(d.bbox, (-140, -140, 140, 140));
        let pos = d.positions();
        assert!(
            pos.iter()
                .all(|&(x, y)| (-140..=140).contains(&x) && (-140..=140).contains(&y))
        );
        assert_eq!(card.to_image().unwrap(), dump[..len]);
    }
}
