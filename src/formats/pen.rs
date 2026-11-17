//! Brother PP1 "Skitch" PEN stream (what the machine receives over
//! Bluetooth): headerless 4-byte records. X and Y are LE16 absolute
//! positions in 0.1 mm, shifted left by 3, with flags in the low 3 bits.
//! Layout and lock-stitch rules follow ZigZag / Respira (Apache-2.0).

use super::{FormatError, err};
use crate::pattern::{Op, Pattern};

/// Y flag: move without stitching.
const FEED: u16 = 0x01;
/// Y flag: cut the thread.
const CUT: u16 = 0x02;
/// X flag value: end of a colour.
const COLOR_END: u16 = 0x03;
/// X flag value: last record.
const DATA_END: u16 = 0x05;
/// Feeds longer than this get lock stitches and a cut.
const FEED_LENGTH: f64 = 50.0;
/// Lock stitches: 8 back-and-forth stitches this far from the anchor.
const LOCK_SIZE: f64 = 2.0;
const LOCK_LOOKAHEAD: usize = 5;
const LOCK_TARGET: f64 = 8.0;

struct Enc {
    out: Vec<u8>,
    /// Stitch positions, for lock directions.
    pts: Vec<(i32, i32)>,
}

impl Enc {
    fn push(&mut self, x: i32, y: i32, xflag: u16, yflag: u16) {
        let xe = ((x << 3) as u16 & 0xFFF8) | xflag;
        let ye = ((y << 3) as u16 & 0xFFF8) | yflag;
        self.out.extend(xe.to_le_bytes());
        self.out.extend(ye.to_le_bytes());
    }

    /// Unit-ish direction from stitch `i` toward its neighbours.
    fn direction(&self, i: usize, ahead: bool) -> (f64, f64) {
        let (cx, cy) = self.pts[i];
        let (mut ax, mut ay) = (0.0, 0.0);
        let mut best = (0.0, 0.0, 0.0);
        for k in 1..=LOCK_LOOKAHEAD {
            let j = if ahead {
                i + k
            } else {
                match i.checked_sub(k) {
                    Some(j) => j,
                    None => break,
                }
            };
            let Some(&(x, y)) = self.pts.get(j) else {
                break;
            };
            ax += (x - cx) as f64;
            ay += (y - cy) as f64;
            let len = (ax * ax + ay * ay).sqrt();
            if len > best.0 {
                best = (len, ax, ay);
            }
            if len >= LOCK_TARGET {
                return (ax / len, ay / len);
            }
        }
        if best.0 > 0.1 {
            (best.1 / best.0, best.2 / best.0)
        } else {
            (
                std::f64::consts::FRAC_1_SQRT_2,
                std::f64::consts::FRAC_1_SQRT_2,
            )
        }
    }

    fn locks(&mut self, x: i32, y: i32, (dx, dy): (f64, f64)) {
        for i in 0..8 {
            let s = if i % 2 == 0 { LOCK_SIZE } else { -LOCK_SIZE };
            self.push(
                (x as f64 + dx * s).round() as i32,
                (y as f64 + dy * s).round() as i32,
                0,
                0,
            );
        }
    }
}

pub fn write(p: &Pattern) -> Vec<u8> {
    let pts: Vec<(i32, i32)> = p
        .cmds
        .iter()
        .filter(|c| c.op == Op::Stitch)
        .map(|c| (c.x, c.y))
        .collect();
    let mut e = Enc {
        out: Vec::new(),
        pts,
    };
    let mut si = 0usize; // index of the next stitch in `pts`
    let mut cur = (0, 0);
    let mut started = false;
    let mut cut_pending = false;
    let mut move_to: Option<(i32, i32)> = None;
    let mut new_color = false;
    for c in &p.cmds {
        match c.op {
            Op::Jump => move_to = Some((c.x, c.y)),
            Op::Trim => cut_pending = true,
            Op::ColorChange if started => {
                e.locks(cur.0, cur.1, e.direction(si - 1, false));
                e.push(cur.0, cur.1, COLOR_END, CUT);
                new_color = true;
                cut_pending = false;
            }
            Op::Stitch => {
                let target = move_to.take();
                if !started {
                    e.push(c.x, c.y, 0, 0);
                    e.locks(c.x, c.y, e.direction(si, true));
                    started = true;
                } else if new_color {
                    // Feed to where the colour starts, lock, then stitch.
                    let at = target.unwrap_or((c.x, c.y));
                    if at != cur {
                        e.push(at.0, at.1, 0, FEED);
                    }
                    e.locks(at.0, at.1, e.direction(si, true));
                    e.push(c.x, c.y, 0, 0);
                    new_color = false;
                } else if let Some((tx, ty)) = target.filter(|&t| t != cur) {
                    let dist = (((tx - cur.0).pow(2) + (ty - cur.1).pow(2)) as f64).sqrt();
                    if cut_pending || dist > FEED_LENGTH {
                        e.locks(cur.0, cur.1, e.direction(si - 1, false));
                        e.push(tx, ty, 0, FEED | CUT);
                        e.locks(tx, ty, e.direction(si, true));
                    } else {
                        e.push(tx, ty, 0, FEED);
                    }
                    cut_pending = false;
                    if (tx, ty) != (c.x, c.y) {
                        e.push(c.x, c.y, 0, 0);
                    }
                } else {
                    e.push(c.x, c.y, 0, 0);
                }
                cur = (c.x, c.y);
                si += 1;
            }
            Op::ColorChange => {}
            Op::End => break,
        }
    }
    // Flag the last record as the end of the data.
    if let Some(rec) = e.out.len().checked_sub(4) {
        e.out[rec] = (e.out[rec] & 0xF8) | DATA_END as u8;
    }
    e.out
}

pub fn read(data: &[u8]) -> Result<Pattern, FormatError> {
    if !data.len().is_multiple_of(4) {
        return err("PEN data is not a whole number of 4-byte records");
    }
    let mut p = Pattern::default();
    for rec in data.as_chunks::<4>().0 {
        let xr = u16::from_le_bytes([rec[0], rec[1]]);
        let yr = u16::from_le_bytes([rec[2], rec[3]]);
        let (x, y) = ((xr as i16 >> 3) as i32, (yr as i16 >> 3) as i32);
        let (xflag, yflag) = (xr & 7, yr & 7);
        if xflag == COLOR_END {
            p.color_change();
            continue;
        }
        if yflag & FEED != 0 {
            if yflag & CUT != 0 {
                p.trim();
            }
            p.jump_to(x, y);
        } else {
            p.stitch_to(x, y);
        }
        if xflag == DATA_END {
            break;
        }
    }
    p.end();
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::testutil::sample;

    #[test]
    fn roundtrip_keeps_design_stitches_in_order() {
        let p = sample();
        let bytes = write(&p);
        assert_eq!(bytes.len() % 4, 0);
        assert_eq!(bytes[bytes.len() - 4] & 7, DATA_END as u8);
        let back = read(&bytes).unwrap();
        assert_eq!(back.color_changes(), 1);
        // Every design stitch appears, in order, among the read-back stitches
        // (which also contain the lock stitches).
        let got: Vec<(i32, i32)> = back
            .cmds
            .iter()
            .filter(|c| c.op == Op::Stitch)
            .map(|c| (c.x, c.y))
            .collect();
        let mut it = got.iter();
        for want in p.cmds.iter().filter(|c| c.op == Op::Stitch) {
            assert!(it.any(|&g| g == (want.x, want.y)), "missing {want:?}");
        }
    }

    #[test]
    fn trim_within_a_colour_becomes_a_cut_feed() {
        let mut p = Pattern::new("t");
        p.stitch_to(0, 0);
        p.stitch_to(10, 0);
        p.trim();
        p.jump_to(20, 20);
        p.stitch_to(25, 20);
        let back = read(&write(&p)).unwrap();
        let trim = back
            .cmds
            .iter()
            .position(|c| c.op == Op::Trim)
            .expect("trim");
        assert_eq!(
            (
                back.cmds[trim + 1].op,
                back.cmds[trim + 1].x,
                back.cmds[trim + 1].y
            ),
            (Op::Jump, 20, 20)
        );
    }

    #[test]
    fn negative_coordinates() {
        let mut p = Pattern::new("n");
        p.stitch_to(-300, -7);
        p.stitch_to(-290, 4);
        let back = read(&write(&p)).unwrap();
        assert_eq!((back.cmds[0].x, back.cmds[0].y), (-300, -7));
    }
}
