//! Melco EXP: headerless 2-byte deltas, y up, `80 xx` control escapes.

use super::FormatError;
use crate::pattern::{Op, Pattern};

pub const MAX_STEP: i32 = 127;

fn delta(dx: i32, dy: i32) -> [u8; 2] {
    [dx as i8 as u8, (-dy) as i8 as u8]
}

pub fn write(p: &Pattern) -> Vec<u8> {
    let p = p.with_max_step(MAX_STEP, MAX_STEP);
    let mut out = Vec::new();
    for (c, dx, dy) in p.deltas() {
        match c.op {
            Op::Stitch => out.extend(delta(dx, dy)),
            Op::Jump => {
                out.extend([0x80, 0x04]);
                out.extend(delta(dx, dy));
            }
            Op::Trim => out.extend([0x80, 0x80, 0x07, 0x00]),
            Op::ColorChange => out.extend([0x80, 0x01, 0x00, 0x00]),
            Op::End => break,
        }
    }
    out
}

pub fn read(data: &[u8]) -> Result<Pattern, FormatError> {
    let mut p = Pattern::default();
    let mut recs = data.as_chunks::<2>().0.iter();
    let d = |b: &[u8]| (b[0] as i8 as i32, -(b[1] as i8 as i32));
    while let Some(b) = recs.next() {
        if b[0] != 0x80 {
            let (dx, dy) = d(b);
            p.stitch(dx, dy);
            continue;
        }
        let Some(arg) = recs.next() else { break };
        let (dx, dy) = d(arg);
        match b[1] {
            0x80 => p.trim(),
            0x02 => p.stitch(dx, dy),
            0x04 => p.jump(dx, dy),
            0x01 => {
                p.color_change();
                if dx != 0 || dy != 0 {
                    p.jump(dx, dy);
                }
            }
            _ => break,
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
    fn roundtrip() {
        let p = sample();
        let back = read(&write(&p)).unwrap();
        assert_eq!(
            stitches(&back),
            stitches(&p.with_max_step(MAX_STEP, MAX_STEP))
        );
        assert_eq!(back.color_changes(), 1);
        assert_eq!(back.cmds.iter().filter(|c| c.op == Op::Trim).count(), 1);
    }
}
