//! Machine embroidery file formats. Every format here both reads and writes;
//! layouts follow pyembroidery (PES/PEC/DST/EXP/JEF/VP3) and ZigZag (PEN).

use std::fmt;
use std::path::Path;

use crate::pattern::Pattern;

pub mod dst;
pub mod exp;
pub mod jef;
pub mod palette;
pub mod pec;
pub mod pen;
pub mod pes;
pub mod vp3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Pes,
    Pec,
    Dst,
    Exp,
    Jef,
    Vp3,
    Pen,
}

pub const ALL: [Format; 7] = [
    Format::Pes,
    Format::Pec,
    Format::Dst,
    Format::Exp,
    Format::Jef,
    Format::Vp3,
    Format::Pen,
];

impl Format {
    pub fn from_ext(ext: &str) -> Option<Format> {
        ALL.into_iter().find(|f| f.ext().eq_ignore_ascii_case(ext))
    }

    pub fn from_path(path: &Path) -> Option<Format> {
        Format::from_ext(path.extension()?.to_str()?)
    }

    pub fn ext(self) -> &'static str {
        match self {
            Format::Pes => "pes",
            Format::Pec => "pec",
            Format::Dst => "dst",
            Format::Exp => "exp",
            Format::Jef => "jef",
            Format::Vp3 => "vp3",
            Format::Pen => "pen",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Format::Pes => "Brother / Baby Lock",
            Format::Pec => "Brother, stitches only",
            Format::Dst => "Tajima",
            Format::Exp => "Melco / Bernina",
            Format::Jef => "Janome / Elna",
            Format::Vp3 => "Husqvarna Viking / Pfaff",
            Format::Pen => "Brother Skitch (PP1)",
        }
    }

    pub fn read(self, data: &[u8]) -> Result<Pattern, FormatError> {
        match self {
            Format::Pes => pes::read(data),
            Format::Pec => pec::read(data),
            Format::Dst => dst::read(data),
            Format::Exp => exp::read(data),
            Format::Jef => jef::read(data),
            Format::Vp3 => vp3::read(data),
            Format::Pen => pen::read(data),
        }
    }

    pub fn write(self, p: &Pattern) -> Result<Vec<u8>, FormatError> {
        match self {
            Format::Pes => pes::write(p),
            Format::Pec => pec::write(p),
            Format::Dst => Ok(dst::write(p)),
            Format::Exp => Ok(exp::write(p)),
            Format::Jef => Ok(jef::write(p)),
            Format::Vp3 => Ok(vp3::write(p)),
            Format::Pen => Ok(pen::write(p)),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct FormatError(pub String);

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FormatError {}

pub(crate) fn err<T>(msg: impl Into<String>) -> Result<T, FormatError> {
    Err(FormatError(msg.into()))
}

/// Read a pattern, choosing the format from the file extension.
pub fn read_file(path: &Path) -> Result<Pattern, Box<dyn std::error::Error>> {
    let fmt = Format::from_path(path)
        .ok_or_else(|| format!("unknown embroidery format: {}", path.display()))?;
    let mut p = fmt.read(&std::fs::read(path)?)?;
    if p.name.is_empty() {
        p.name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
    }
    Ok(p)
}

/// Write a pattern, choosing the format from the file extension.
pub fn write_file(path: &Path, p: &Pattern) -> Result<(), Box<dyn std::error::Error>> {
    let fmt = Format::from_path(path)
        .ok_or_else(|| format!("unknown embroidery format: {}", path.display()))?;
    std::fs::write(path, fmt.write(p)?)?;
    Ok(())
}

/// Little cursor for the readers.
pub(crate) struct Reader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }

    pub fn at(data: &'a [u8], pos: usize) -> Self {
        Reader { data, pos }
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], FormatError> {
        let s = self
            .data
            .get(self.pos..self.pos + n)
            .ok_or_else(|| FormatError("unexpected end of file".into()))?;
        self.pos += n;
        Ok(s)
    }

    pub fn skip(&mut self, n: usize) -> Result<(), FormatError> {
        self.bytes(n).map(drop)
    }

    pub fn u8(&mut self) -> Result<u8, FormatError> {
        Ok(self.bytes(1)?[0])
    }

    pub fn u16be(&mut self) -> Result<u16, FormatError> {
        Ok(u16::from_be_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    pub fn u24le(&mut self) -> Result<u32, FormatError> {
        let b = self.bytes(3)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], 0]))
    }

    pub fn u24be(&mut self) -> Result<u32, FormatError> {
        let b = self.bytes(3)?;
        Ok(u32::from_be_bytes([0, b[0], b[1], b[2]]))
    }

    pub fn u32le(&mut self) -> Result<u32, FormatError> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    pub fn u32be(&mut self) -> Result<u32, FormatError> {
        Ok(u32::from_be_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    pub fn i32be(&mut self) -> Result<i32, FormatError> {
        Ok(i32::from_be_bytes(self.bytes(4)?.try_into().unwrap()))
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use crate::pattern::{Op, Pattern, Thread};

    /// A small two-colour pattern with long moves, a trim and a jump.
    pub fn sample() -> Pattern {
        let mut p = Pattern::new("sample");
        p.threads = vec![Thread::rgb(0xED171F), Thread::rgb(0x0A55A3)];
        p.stitch_to(0, 0);
        for i in 0..20 {
            p.stitch(if i % 2 == 0 { 30 } else { -25 }, 12);
        }
        p.stitch(200, -40);
        p.trim();
        p.jump(-150, 90);
        p.color_change();
        for i in 0..15 {
            p.stitch(-8, if i % 2 == 0 { -50 } else { 45 });
        }
        p.end();
        p
    }

    /// Stitch positions only, the property every format must preserve.
    pub fn stitches(p: &Pattern) -> Vec<(i32, i32)> {
        p.cmds
            .iter()
            .filter(|c| c.op == Op::Stitch)
            .map(|c| (c.x, c.y))
            .collect()
    }

    /// Stitch positions with consecutive duplicates removed (some formats
    /// add zero-length tie stitches).
    pub fn path(p: &Pattern) -> Vec<(i32, i32)> {
        let mut v = stitches(p);
        v.dedup();
        v
    }

    /// `got` is `want` plus anchor stitches placed only on jump landings.
    pub fn assert_path_with_anchors(want: &Pattern, got: &Pattern) {
        let landings: Vec<(i32, i32)> = want
            .cmds
            .iter()
            .filter(|c| c.op == Op::Jump)
            .map(|c| (c.x, c.y))
            .collect();
        let extra: Vec<(i32, i32)> = {
            let mut w = path(want).into_iter().peekable();
            path(got)
                .into_iter()
                .filter(|g| {
                    if w.peek() == Some(g) {
                        w.next();
                        false
                    } else {
                        true
                    }
                })
                .collect()
        };
        assert!(
            extra.iter().all(|e| landings.contains(e)),
            "unexpected extra stitches {extra:?}"
        );
        let mut it = path(got).into_iter();
        for s in path(want) {
            assert!(it.any(|g| g == s), "missing stitch {s:?}");
        }
    }
}
