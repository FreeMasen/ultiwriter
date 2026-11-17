//! Brother PES (version 1): a CEmbOne/CSewSeg block for editing software,
//! followed by the PEC section machines actually sew from.

use super::pec;
use super::{FormatError, Reader, err};
use crate::pattern::{Op, Pattern};

fn string16(out: &mut Vec<u8>, s: &str) {
    out.extend((s.len() as u16).to_le_bytes());
    out.extend(s.as_bytes());
}

fn i16le(out: &mut Vec<u8>, v: i32) {
    out.extend((v as i16).to_le_bytes());
}

pub fn write(p: &Pattern) -> Result<Vec<u8>, FormatError> {
    let p = p.with_max_step(pec::MAX_STEP, pec::MAX_STEP);
    let mut out = b"#PES0001".to_vec();
    let pec_offset_at = out.len();
    out.extend([0; 4]);
    let has_stitches = p.stitch_count() > 0;
    // Header: scale to fit, 130x180 hoop, number of block objects.
    for v in [1, 1, has_stitches as i32] {
        i16le(&mut out, v);
    }
    if has_stitches {
        out.extend([0xFF, 0xFF, 0, 0]);
        write_blocks(&p, &mut out);
    } else {
        out.extend([0, 0, 0, 0]);
    }
    let pec_at = out.len() as u32;
    out[pec_offset_at..pec_offset_at + 4].copy_from_slice(&pec_at.to_le_bytes());
    pec::write_pec(&p, &mut out)?;
    Ok(out)
}

fn write_blocks(p: &Pattern, out: &mut Vec<u8>) {
    let (x0, y0, x1, y1) = p.bounds();
    let (w, h) = (x1 - x0, y1 - y0);
    string16(out, "CEmbOne");
    out.extend([0; 16]); // two empty extent rectangles
    // Affine transform placing the design in a 130x180 mm hoop.
    let tx = 350.0 + 1300.0 / 2.0 - w as f32 / 2.0;
    let ty = 100.0 + h as f32 + 1800.0 / 2.0 - h as f32 / 2.0;
    for v in [1.0f32, 0.0, 0.0, 1.0, tx, ty] {
        out.extend(v.to_le_bytes());
    }
    for v in [1, 0, 0, w, h] {
        i16le(out, v);
    }
    out.extend([0; 8]);
    let sections_at = out.len();
    out.extend([0, 0]);
    out.extend([0xFF, 0xFF, 0, 0]); // more blocks follow
    string16(out, "CSewSeg");

    // Segments: runs of stitches (flag 0) and jumps (flag 1), relative to
    // the bottom-left of the design.
    let colors = pec::palette(p);
    let (ax, ay) = (x0, y1);
    let mut color_i = 0usize;
    let mut log: Vec<(u16, u16)> = Vec::new();
    let mut sections = 0u16;
    let mut last = (0, 0);
    let mut i = 0;
    let cmds = &p.cmds;
    while i < cmds.len() {
        let op = cmds[i].op;
        let run_end = (i..cmds.len())
            .find(|&j| cmds[j].op != op)
            .unwrap_or(cmds.len());
        let (flag, pts): (u16, Vec<(i32, i32)>) = match op {
            Op::Stitch => (0, cmds[i..run_end].iter().map(|c| (c.x, c.y)).collect()),
            Op::Jump => (1, vec![last, (cmds[run_end - 1].x, cmds[run_end - 1].y)]),
            Op::ColorChange => {
                color_i += run_end - i;
                i = run_end;
                continue;
            }
            _ => {
                i = run_end;
                continue;
            }
        };
        if op == Op::Stitch {
            last = *pts.last().unwrap();
        }
        let code = colors[color_i.min(colors.len() - 1)] as u16;
        if sections > 0 {
            out.extend(0x8003u16.to_le_bytes());
        }
        if log.last().is_none_or(|&(_, c)| c != code) {
            log.push((sections, code));
        }
        out.extend(flag.to_le_bytes());
        out.extend(code.to_le_bytes());
        out.extend((pts.len() as u16).to_le_bytes());
        for (x, y) in pts {
            i16le(out, x - ax);
            i16le(out, y - ay);
        }
        sections += 1;
        i = run_end;
    }
    out.extend((log.len() as u16).to_le_bytes());
    for (s, c) in &log {
        out.extend(s.to_le_bytes());
        out.extend(c.to_le_bytes());
    }
    out[sections_at..sections_at + 2].copy_from_slice(&sections.to_le_bytes());
    out.extend([0, 0, 0, 0]); // no more blocks
}

/// Read any PES version through its PEC section (thread colours come from
/// the PEC palette; v5+ RGB charts are not used).
pub fn read(data: &[u8]) -> Result<Pattern, FormatError> {
    if !data.starts_with(b"#PES") {
        return err("not a PES file (missing #PES signature)");
    }
    let mut r = Reader::at(data, 8);
    let pec_at = r.u32le()? as usize;
    if pec_at >= data.len() {
        return err("PES file's PEC offset points past the end");
    }
    pec::read_pec(data, pec_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::testutil::{assert_path_with_anchors, sample};

    #[test]
    fn roundtrip() {
        let p = sample();
        let bytes = write(&p).unwrap();
        assert!(bytes.starts_with(b"#PES0001"));
        let back = read(&bytes).unwrap();
        assert_path_with_anchors(&p, &back);
        assert_eq!(back.color_changes(), 1);
    }
}
