//! Brother PEC: label + palette header, a stitch block of 7/12-bit deltas
//! (y down), then 48x38 monochrome thumbnails. Also the core of PES.

use super::palette::{PEC_THREADS, rgbs};
use super::{FormatError, Reader, err};
use crate::pattern::{Op, Pattern, Thread, nearest_color};

pub const MAX_STEP: i32 = 2047;
const ICON_W: usize = 48;
const ICON_H: usize = 38;
const STRIDE: usize = ICON_W / 8;
const FLAG_LONG: u8 = 0x80;
const JUMP_CODE: u8 = 0x10;
const TRIM_CODE: u8 = 0x20;

pub fn write(p: &Pattern) -> Result<Vec<u8>, FormatError> {
    let mut out = b"#PEC0001".to_vec();
    write_pec(p, &mut out)?;
    Ok(out)
}

/// PEC palette indices for each colour block.
pub fn palette(p: &Pattern) -> Vec<u8> {
    let chart = rgbs(PEC_THREADS);
    (0..p.color_count())
        .map(|i| nearest_color(p.thread(i).rgb, &chart) as u8)
        .collect()
}

/// Append the PEC section (header, stitches, thumbnails) to `out`.
pub fn write_pec(p: &Pattern, out: &mut Vec<u8>) -> Result<(), FormatError> {
    let p = p.with_max_step(MAX_STEP, MAX_STEP).jumps_land_on_stitches();
    let colors = palette(&p);
    if colors.len() > 255 {
        return err("PEC holds at most 255 colours");
    }
    let start = out.len();
    out.extend(format!("LA:{:<16}\r", p.name.chars().take(8).collect::<String>()).as_bytes());
    out.extend(b"            \xFF\x00");
    out.extend([STRIDE as u8, ICON_H as u8]);
    out.extend(b"            ");
    out.push(colors.len() as u8 - 1);
    out.extend(&colors);
    out.resize(start + 512, b' ');

    let (x0, y0, x1, y1) = p.bounds();
    let block = out.len();
    out.extend([0, 0, 0, 0, 0, 0x31, 0xFF, 0xF0]);
    out.extend(((x1 - x0) as u16).to_le_bytes());
    out.extend(((y1 - y0) as u16).to_le_bytes());
    out.extend(0x1E0u16.to_le_bytes());
    out.extend(0x1B0u16.to_le_bytes());
    encode(&p, out);
    let len = (out.len() - block) as u32;
    out[block + 2..block + 5].copy_from_slice(&len.to_le_bytes()[..3]);

    // Thumbnails: all stitches, then one per colour.
    let bounds = (x0, y0, x1, y1);
    let all: Vec<(i32, i32)> = p
        .cmds
        .iter()
        .filter(|c| c.op == Op::Stitch)
        .map(|c| (c.x, c.y))
        .collect();
    out.extend(icon(&all, bounds));
    for b in p.color_blocks() {
        let pts: Vec<(i32, i32)> = b
            .iter()
            .filter(|c| c.op == Op::Stitch)
            .map(|c| (c.x, c.y))
            .collect();
        out.extend(icon(&pts, bounds));
    }
    Ok(())
}

fn value(out: &mut Vec<u8>, v: i32, long: bool, flag: u8) {
    if !long && -64 < v && v < 63 {
        out.push((v & 0x7F) as u8);
    } else {
        let v = (v as u16 & 0x0FFF) | 0x8000 | ((flag as u16) << 8);
        out.extend(v.to_be_bytes());
    }
}

fn encode(p: &Pattern, out: &mut Vec<u8>) {
    let (mut jumping, mut first, mut color_two) = (true, true, true);
    for (c, dx, dy) in p.deltas() {
        match c.op {
            // Open with a jump to the first stitch rather than sewing from the origin.
            Op::Stitch if first && (dx, dy) != (0, 0) => {
                value(out, dx, true, JUMP_CODE);
                value(out, dy, true, JUMP_CODE);
                value(out, 0, false, 0);
                value(out, 0, false, 0);
                jumping = false;
            }
            Op::Stitch => {
                // Brother machines expect a zero stitch to anchor after a jump.
                if jumping && dx != 0 && dy != 0 {
                    value(out, 0, false, 0);
                    value(out, 0, false, 0);
                }
                jumping = false;
                value(out, dx, false, 0);
                value(out, dy, false, 0);
            }
            Op::Jump => {
                jumping = true;
                let flag = if first { JUMP_CODE } else { TRIM_CODE };
                value(out, dx, true, flag);
                value(out, dy, true, flag);
            }
            Op::ColorChange => {
                if jumping {
                    value(out, 0, false, 0);
                    value(out, 0, false, 0);
                    jumping = false;
                }
                out.extend([0xFE, 0xB0, if color_two { 2 } else { 1 }]);
                color_two = !color_two;
            }
            Op::Trim => {}
            Op::End => break,
        }
        first = false;
    }
    out.push(0xFF);
}

/// 48x38 one-bit thumbnail: a frame plus the stitches scaled to fit. Row 0
/// stays blank: readers find the end of the stitch data as `FF 00`.
fn icon(pts: &[(i32, i32)], (x0, y0, x1, y1): (i32, i32, i32, i32)) -> Vec<u8> {
    let mut g = vec![0u8; STRIDE * ICON_H];
    let mut set = |x: usize, y: usize| {
        if x < ICON_W && y < ICON_H {
            g[y * STRIDE + x / 8] |= 1 << (x % 8);
        }
    };
    for x in 4..ICON_W - 4 {
        set(x, 1);
        set(x, ICON_H - 1);
    }
    for y in 3..ICON_H - 2 {
        set(0, y);
        set(ICON_W - 1, y);
    }
    let (w, h) = ((x1 - x0).max(1) as f64, (y1 - y0).max(1) as f64);
    let scale = ((ICON_W - 10) as f64 / w).min((ICON_H - 10) as f64 / h);
    let (cx, cy) = ((x0 + x1) as f64 / 2.0, (y0 + y1) as f64 / 2.0);
    for &(x, y) in pts {
        let gx = ((x as f64 - cx) * scale + ICON_W as f64 / 2.0).floor() as usize;
        let gy = ((y as f64 - cy) * scale + ICON_H as f64 / 2.0).floor() as usize;
        set(gx, gy);
    }
    g
}

pub fn read(data: &[u8]) -> Result<Pattern, FormatError> {
    if !data.starts_with(b"#PEC") {
        return err("not a PEC file (missing #PEC signature)");
    }
    read_pec(data, 8)
}

fn signed12(v: u16) -> i32 {
    let v = (v & 0x0FFF) as i32;
    if v > 0x7FF { v - 0x1000 } else { v }
}

fn signed7(v: u8) -> i32 {
    if v > 63 { v as i32 - 128 } else { v as i32 }
}

/// Parse a PEC section starting at `at` (its `LA:` label).
pub fn read_pec(data: &[u8], at: usize) -> Result<Pattern, FormatError> {
    let mut r = Reader::at(data, at);
    if r.bytes(3)? != b"LA:" {
        return err("PEC section lacks its LA: label");
    }
    let mut p = Pattern {
        name: String::from_utf8_lossy(r.bytes(16)?).trim().to_string(),
        ..Default::default()
    };
    r.skip(0x0F + 2 + 0x0C)?;
    let count = r.u8()? as usize + 1;
    for &i in r.bytes(count)? {
        let t = &PEC_THREADS[i as usize % PEC_THREADS.len()];
        p.threads.push(Thread {
            rgb: t.rgb,
            name: t.name.into(),
            catalog: t.code.into(),
            brand: "Brother".into(),
        });
    }
    // The stitch block starts 512 bytes into the section.
    let mut r = Reader::at(data, at + 512 + 2);
    let len = r.u24le()? as usize;
    let end = (at + 512 + len).min(data.len());
    r.skip(0x0B)?;
    while r.pos < end {
        let v1 = r.u8()?;
        if v1 == 0xFF {
            break;
        }
        let v2 = r.u8()?;
        if v1 == 0xFE && v2 == 0xB0 {
            r.skip(1)?;
            p.color_change();
            continue;
        }
        let (mut jump, mut trim) = (false, false);
        let mut flags = |b: u8| {
            jump |= b & JUMP_CODE != 0;
            trim |= b & TRIM_CODE != 0;
        };
        let (x, yb) = if v1 & FLAG_LONG != 0 {
            flags(v1);
            (signed12(u16::from_be_bytes([v1, v2])), r.u8()?)
        } else {
            (signed7(v1), v2)
        };
        let y = if yb & FLAG_LONG != 0 {
            flags(yb);
            signed12(u16::from_be_bytes([yb, r.u8()?]))
        } else {
            signed7(yb)
        };
        if trim {
            p.trim();
            p.jump(x, y);
        } else if jump {
            p.jump(x, y);
        } else {
            p.stitch(x, y);
        }
    }
    p.end();
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::testutil::{assert_path_with_anchors, sample};

    #[test]
    fn roundtrip() {
        let p = sample();
        let bytes = write(&p).unwrap();
        let back = read(&bytes).unwrap();
        assert_path_with_anchors(&p, &back);
        assert_eq!(back.color_changes(), 1);
        assert_eq!(back.name, "sample");
        assert_eq!(back.threads.len(), 2);
        // Red and blue map to Brother "Red" and "Blue".
        assert_eq!(
            (back.threads[0].name.as_str(), back.threads[1].name.as_str()),
            ("Red", "Blue")
        );
    }

    #[test]
    fn long_and_short_values() {
        let mut v = Vec::new();
        value(&mut v, 62, false, 0);
        value(&mut v, 63, false, 0);
        value(&mut v, -2000, true, JUMP_CODE);
        assert_eq!(v[0], 62);
        assert_eq!(signed12(u16::from_be_bytes([v[1], v[2]])), 63);
        assert_eq!(v[3] & (FLAG_LONG | JUMP_CODE), FLAG_LONG | JUMP_CODE);
        assert_eq!(signed12(u16::from_be_bytes([v[3], v[4]])), -2000);
    }
}
