//! Tajima DST: 512-byte text header, then 3-byte records, y up.

use super::{FormatError, err};
use crate::pattern::{Op, Pattern};

pub const MAX_STEP: i32 = 121;
const HEADER: usize = 512;
/// Jumps in a row that read back as a trim (pyembroidery's default).
const TRIM_JUMPS: usize = 3;

/// Bit for each balanced-ternary weight: (byte, bit for +, bit for -).
const X_BITS: [(i32, usize, u8, u8); 5] = [
    (81, 2, 2, 3),
    (27, 1, 2, 3),
    (9, 0, 2, 3),
    (3, 1, 0, 1),
    (1, 0, 0, 1),
];
const Y_BITS: [(i32, usize, u8, u8); 5] = [
    (81, 2, 5, 4),
    (27, 1, 5, 4),
    (9, 0, 5, 4),
    (3, 1, 7, 6),
    (1, 0, 7, 6),
];

fn encode(dx: i32, dy: i32, jump: bool) -> [u8; 3] {
    let mut b = [0u8, 0, 0b11];
    if jump {
        b[2] |= 0x80;
    }
    for (bits, mut v) in [(&X_BITS, dx), (&Y_BITS, -dy)] {
        for &(w, byte, plus, minus) in bits {
            // Greedy balanced ternary: take a weight when |v| exceeds half of it.
            let half = w / 2;
            if v > half {
                b[byte] |= 1 << plus;
                v -= w;
            } else if v < -half {
                b[byte] |= 1 << minus;
                v += w;
            }
        }
        debug_assert_eq!(v, 0, "DST step out of range");
    }
    b
}

fn decode(b: &[u8]) -> (i32, i32) {
    let get = |bits: &[(i32, usize, u8, u8); 5]| {
        bits.iter()
            .map(|&(w, byte, plus, minus)| {
                w * (((b[byte] >> plus) & 1) as i32 - ((b[byte] >> minus) & 1) as i32)
            })
            .sum::<i32>()
    };
    (get(&X_BITS), -get(&Y_BITS))
}

pub fn write(p: &Pattern) -> Vec<u8> {
    let p = p.with_max_step(MAX_STEP, MAX_STEP);
    let (x0, y0, x1, y1) = p.bounds();
    let (ax, ay) = p.position();
    let mut h = String::new();
    h += &format!("LA:{:<16}\r", p.name.chars().take(16).collect::<String>());
    h += &format!("ST:{:7}\r", p.cmds.len());
    h += &format!("CO:{:3}\r", p.color_changes());
    h += &format!(
        "+X:{:5}\r-X:{:5}\r+Y:{:5}\r-Y:{:5}\r",
        x1.abs(),
        x0.abs(),
        y1.abs(),
        y0.abs()
    );
    let signed = |v: i32| format!("{}{:5}", if v >= 0 { '+' } else { '-' }, v.abs());
    h += &format!("AX:{}\rAY:{}\r", signed(ax), signed(-ay));
    h += "MX:+    0\rMY:+    0\rPD:******\r\x1a";
    let mut out = h.into_bytes();
    out.resize(HEADER, b' ');

    for (c, dx, dy) in p.deltas() {
        match c.op {
            Op::Stitch => out.extend(encode(dx, dy, false)),
            Op::Jump => out.extend(encode(dx, dy, true)),
            // A trim is a little jump dance that returns to the same spot.
            Op::Trim => {
                out.extend(encode(2, 2, true));
                out.extend(encode(-4, -4, true));
                out.extend(encode(2, 2, true));
            }
            Op::ColorChange => out.extend([0, 0, 0xC3]),
            Op::End => break,
        }
    }
    out.extend([0, 0, 0xF3]);
    out
}

pub fn read(data: &[u8]) -> Result<Pattern, FormatError> {
    if data.len() < HEADER {
        return err("DST file shorter than its 512-byte header");
    }
    let mut p = Pattern::default();
    for line in data[..HEADER].split(|&b| b == b'\r' || b == b'\n') {
        if let Some(name) = line.strip_prefix(b"LA:") {
            p.name = String::from_utf8_lossy(name).trim().to_string();
        }
    }
    let mut jumps = 0;
    for rec in data[HEADER..].as_chunks::<3>().0 {
        let (dx, dy) = decode(rec);
        let ctl = rec[2];
        if ctl & 0xF3 == 0xF3 {
            break;
        }
        if ctl & 0xC3 == 0xC3 {
            p.color_change();
            if dx != 0 || dy != 0 {
                p.jump(dx, dy);
            }
            jumps = 0;
        } else if ctl & 0x83 == 0x83 {
            p.jump(dx, dy);
            jumps += 1;
            if jumps == TRIM_JUMPS {
                // Mark the trim before the run of jumps that signalled it.
                let at = p.cmds.len() - TRIM_JUMPS;
                let (x, y) = if at == 0 {
                    (0, 0)
                } else {
                    (p.cmds[at - 1].x, p.cmds[at - 1].y)
                };
                p.cmds
                    .insert(at, crate::pattern::Cmd { op: Op::Trim, x, y });
            }
        } else {
            p.stitch(dx, dy);
            jumps = 0;
        }
    }
    p.end();
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::testutil::{sample, stitches};

    #[test]
    fn ternary_roundtrip_every_step() {
        for dx in -MAX_STEP..=MAX_STEP {
            for dy in [-121, -40, -1, 0, 1, 13, 121] {
                assert_eq!(decode(&encode(dx, dy, false)), (dx, dy), "{dx},{dy}");
            }
        }
    }

    #[test]
    fn file_roundtrip() {
        let p = sample();
        let back = read(&write(&p)).unwrap();
        assert_eq!(
            stitches(&back),
            stitches(&p.with_max_step(MAX_STEP, MAX_STEP))
        );
        assert_eq!(back.color_changes(), 1);
        assert!(back.cmds.iter().any(|c| c.op == Op::Trim));
        assert_eq!(back.name, "sample");
    }
}
