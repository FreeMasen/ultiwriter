//! Husqvarna Viking / Pfaff VP3: nested big-endian blocks with back-patched
//! lengths. Positions in 0.001 mm; stitches as in the Viking card format
//! (`dx dy`, long `80 01 dx16 dy16 80 02`, trim `80 03`), y down.

use super::{FormatError, Reader, err};
use crate::pattern::{Op, Pattern, Thread};

const PRODUCER: &str = "Produced by     Software Ltd";

fn string16(out: &mut Vec<u8>, s: &str) {
    let units: Vec<u16> = s.encode_utf16().collect();
    out.extend(((units.len() * 2) as u16).to_be_bytes());
    for u in units {
        out.extend(u.to_be_bytes());
    }
}

fn string8(out: &mut Vec<u8>, s: &str) {
    out.extend((s.len() as u16).to_be_bytes());
    out.extend(s.as_bytes());
}

fn i32be(out: &mut Vec<u8>, v: i32) {
    out.extend(v.to_be_bytes());
}

/// Start a `tag` block; returns where to patch its length.
fn open(out: &mut Vec<u8>, tag: [u8; 3]) -> usize {
    out.extend(tag);
    out.extend([0; 4]);
    out.len() - 4
}

fn close(out: &mut [u8], at: usize) {
    let len = (out.len() - at - 4) as u32;
    out[at..at + 4].copy_from_slice(&len.to_be_bytes());
}

pub fn write(p: &Pattern) -> Vec<u8> {
    let mut out = b"%vsm%\0".to_vec();
    string16(&mut out, PRODUCER);
    let file = open(&mut out, [0, 2, 0]);
    string16(&mut out, "");
    let (x0, y0, x1, y1) = p.bounds();
    for v in [x1 * 100, -y0 * 100, x0 * 100, -y1 * 100] {
        i32be(&mut out, v);
    }
    let blocks = p.color_blocks();
    let ends = p.cmds.iter().filter(|c| c.op == Op::End).count();
    i32be(&mut out, (p.cmds.len() - ends) as i32);
    out.extend([0, blocks.len() as u8, 12, 0, 1]);

    let design = open(&mut out, [0, 3, 0]);
    let (w, h) = (x1 - x0, y1 - y0);
    // Integer centre, so block starts relative to it are exact.
    let (hw, hh) = (w / 2, h / 2);
    let (cx, cy) = (x0 + hw, y0 + hh);
    i32be(&mut out, cx * 100);
    i32be(&mut out, -cy * 100);
    out.extend([0, 0, 0]);
    for v in [-hw * 100, hw * 100, -hh * 100, hh * 100, w * 100, h * 100] {
        i32be(&mut out, v);
    }
    string16(&mut out, "");
    out.extend([0x64, 0x64]);
    for v in [4096, 0, 0, 4096] {
        i32be(&mut out, v);
    }
    out.extend(b"xxPP\x01\x00");
    string16(&mut out, PRODUCER);
    out.extend((blocks.len() as u16).to_be_bytes());

    // Each block (after the first) begins at the position of its colour change.
    let mut starts = vec![(0, 0)];
    starts.extend(
        p.cmds
            .iter()
            .filter(|c| c.op == Op::ColorChange)
            .map(|c| (c.x, c.y)),
    );
    for (i, cmds) in blocks.iter().enumerate() {
        let (fx, fy) = starts[i];
        let (lx, ly) = cmds.last().map_or((fx, fy), |c| (c.x, c.y));
        let block = open(&mut out, [0, 5, 0]);
        i32be(&mut out, (fx - cx) * 100);
        i32be(&mut out, -(fy - cy) * 100);
        let t = p.thread(i);
        out.extend([1, 0]); // one colour, no transition
        out.extend(&t.rgb.to_be_bytes()[1..]);
        out.extend([0, 0, 0, 5, 0x28]); // no parts, no length, rayon 40
        string8(&mut out, &t.catalog);
        string8(
            &mut out,
            if t.name.is_empty() {
                t.hex()
            } else {
                t.name.clone()
            }
            .as_str(),
        );
        string8(&mut out, &t.brand);
        i32be(&mut out, (lx - fx) * 100);
        i32be(&mut out, -(ly - fy) * 100);
        let stitches = open(&mut out, [0, 1, 0]);
        out.extend([0x0A, 0xF6, 0x00]);
        let (mut px, mut py) = (fx, fy);
        for c in cmds.iter() {
            match c.op {
                Op::Stitch => {
                    let (dx, dy) = (c.x - px, c.y - py);
                    (px, py) = (c.x, c.y);
                    if (-127..=127).contains(&dx) && (-127..=127).contains(&dy) {
                        out.extend([dx as i8 as u8, dy as i8 as u8]);
                    } else {
                        out.extend([0x80, 0x01]);
                        out.extend((dx as i16).to_be_bytes());
                        out.extend((dy as i16).to_be_bytes());
                        out.extend([0x80, 0x02]);
                    }
                }
                Op::Trim => out.extend([0x80, 0x03]),
                Op::End => {
                    out.extend([0x80, 0x03]);
                    break;
                }
                // Jumps fold into the next stitch's delta.
                Op::Jump | Op::ColorChange => {}
            }
        }
        close(&mut out, stitches);
        out.push(0);
        close(&mut out, block);
    }
    close(&mut out, design);
    close(&mut out, file);
    out
}

fn skip_string(r: &mut Reader) -> Result<(), FormatError> {
    let n = r.u16be()? as usize;
    r.skip(n)
}

fn read_string8(r: &mut Reader) -> Result<String, FormatError> {
    let n = r.u16be()? as usize;
    Ok(String::from_utf8_lossy(r.bytes(n)?).into_owned())
}

pub fn read(data: &[u8]) -> Result<Pattern, FormatError> {
    if !data.starts_with(b"%vsm%") {
        return err("not a VP3 file (missing %vsm% signature)");
    }
    let mut r = Reader::at(data, 6);
    skip_string(&mut r)?;
    r.skip(7)?;
    skip_string(&mut r)?;
    r.skip(32)?;
    let cx = r.i32be()? / 100;
    let cy = -(r.i32be()? / 100);
    r.skip(27)?;
    skip_string(&mut r)?;
    r.skip(24)?;
    skip_string(&mut r)?;
    let count = r.u16be()? as usize;

    let mut p = Pattern::default();
    for i in 0..count {
        if r.bytes(3)? != [0, 5, 0] {
            return err(format!("VP3 colour block {i} has a bad tag"));
        }
        let len = r.u32be()? as usize;
        let end = (r.pos + len).min(data.len());
        let sx = r.i32be()? / 100 + cx;
        let sy = -(r.i32be()? / 100) + cy;
        if (sx, sy) != p.position() {
            p.jump_to(sx, sy);
        }
        let colors = r.u8()? as usize;
        r.skip(1)?;
        let mut rgb = 0;
        for _ in 0..colors {
            rgb = r.u24be()?;
            r.skip(3)?;
        }
        r.skip(2)?;
        let catalog = read_string8(&mut r)?;
        let name = read_string8(&mut r)?;
        let brand = read_string8(&mut r)?;
        p.threads.push(Thread {
            rgb,
            name,
            catalog,
            brand,
        });
        r.skip(15)?;
        if r.bytes(3)? != [0x0A, 0xF6, 0x00] {
            return err(format!("VP3 stitch block {i} has a bad header"));
        }
        let s = &data[r.pos..end];
        let mut k = 0;
        while k + 1 < s.len() {
            let (a, b) = (s[k], s[k + 1]);
            k += 2;
            if a != 0x80 {
                p.stitch(a as i8 as i32, b as i8 as i32);
                continue;
            }
            match b {
                0x01 if k + 4 <= s.len() => {
                    let dx = i16::from_be_bytes([s[k], s[k + 1]]) as i32;
                    let dy = i16::from_be_bytes([s[k + 2], s[k + 3]]) as i32;
                    p.stitch(dx, dy);
                    k += 6; // value pair + the closing 80 02
                }
                0x03 => p.trim(),
                _ => {}
            }
        }
        r.pos = end;
        if i + 1 < count {
            p.color_change();
        }
    }
    // A trailing trim is the writer's end marker.
    if p.cmds.last().is_some_and(|c| c.op == Op::Trim) {
        p.cmds.pop();
    }
    p.end();
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::testutil::{sample, stitches};

    #[test]
    fn roundtrip_keeps_threads_and_stitches() {
        let mut p = sample();
        p.threads[0].name = "Poppy".into();
        p.threads[0].brand = "Robison-Anton".into();
        let back = read(&write(&p)).unwrap();
        assert_eq!(stitches(&back), stitches(&p));
        assert_eq!(back.color_changes(), 1);
        assert_eq!(back.threads[0].rgb, 0xED171F);
        assert_eq!(back.threads[0].name, "Poppy");
        assert_eq!(back.threads[1].name, "#0a55a3");
    }
}
