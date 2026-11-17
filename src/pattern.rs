//! Format-neutral embroidery pattern: a flat command stream at absolute
//! positions in 0.1 mm, y pointing down (the convention pyembroidery uses).

use crate::card::{self, ColorBlock, Design, Stitch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Stitch,
    /// Move without stitching.
    Jump,
    /// Cut the thread at the current position.
    Trim,
    /// Switch to the next thread at the current position.
    ColorChange,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cmd {
    pub op: Op,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Thread {
    pub rgb: u32,
    pub name: String,
    pub catalog: String,
    pub brand: String,
}

impl Thread {
    pub fn rgb(rgb: u32) -> Self {
        Thread {
            rgb,
            ..Default::default()
        }
    }

    pub fn hex(&self) -> String {
        format!("#{:06x}", self.rgb & 0xFF_FFFF)
    }
}

/// Distinct fallback colours for patterns that name fewer threads than they use.
const FALLBACK: [u32; 12] = [
    0x1F3F9F, 0xC0262D, 0x2E8B3A, 0xE8A400, 0x6B2E91, 0x00879A, 0x000000, 0xF07CB0, 0x7A4A1E,
    0x7F7F7F, 0x9ACD32, 0xFF7F00,
];

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Pattern {
    pub name: String,
    pub cmds: Vec<Cmd>,
    pub threads: Vec<Thread>,
}

impl Pattern {
    pub fn new(name: impl Into<String>) -> Self {
        Pattern {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Current needle position (end of the last command).
    pub fn position(&self) -> (i32, i32) {
        self.cmds.last().map_or((0, 0), |c| (c.x, c.y))
    }

    fn push(&mut self, op: Op, x: i32, y: i32) {
        self.cmds.push(Cmd { op, x, y });
    }

    pub fn stitch_to(&mut self, x: i32, y: i32) {
        self.push(Op::Stitch, x, y);
    }

    pub fn jump_to(&mut self, x: i32, y: i32) {
        self.push(Op::Jump, x, y);
    }

    pub fn stitch(&mut self, dx: i32, dy: i32) {
        let (x, y) = self.position();
        self.stitch_to(x + dx, y + dy);
    }

    pub fn jump(&mut self, dx: i32, dy: i32) {
        let (x, y) = self.position();
        self.jump_to(x + dx, y + dy);
    }

    pub fn trim(&mut self) {
        let (x, y) = self.position();
        self.push(Op::Trim, x, y);
    }

    pub fn color_change(&mut self) {
        let (x, y) = self.position();
        self.push(Op::ColorChange, x, y);
    }

    pub fn end(&mut self) {
        if self.cmds.last().is_none_or(|c| c.op != Op::End) {
            let (x, y) = self.position();
            self.push(Op::End, x, y);
        }
    }

    pub fn stitch_count(&self) -> usize {
        self.cmds.iter().filter(|c| c.op == Op::Stitch).count()
    }

    pub fn color_changes(&self) -> usize {
        self.cmds.iter().filter(|c| c.op == Op::ColorChange).count()
    }

    /// Number of thread colours used (colour changes + 1).
    pub fn color_count(&self) -> usize {
        self.color_changes() + 1
    }

    /// Thread for colour block `i`, falling back to a stock colour.
    pub fn thread(&self, i: usize) -> Thread {
        self.threads
            .get(i)
            .cloned()
            .unwrap_or_else(|| Thread::rgb(FALLBACK[i % FALLBACK.len()]))
    }

    /// (min x, min y, max x, max y) over stitches and jumps; zero if empty.
    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        let mut it = self
            .cmds
            .iter()
            .filter(|c| matches!(c.op, Op::Stitch | Op::Jump));
        let Some(first) = it.next() else {
            return (0, 0, 0, 0);
        };
        it.fold(
            (first.x, first.y, first.x, first.y),
            |(x0, y0, x1, y1), c| (x0.min(c.x), y0.min(c.y), x1.max(c.x), y1.max(c.y)),
        )
    }

    /// Commands grouped by colour, colour-change commands removed.
    pub fn color_blocks(&self) -> Vec<&[Cmd]> {
        self.cmds.split(|c| c.op == Op::ColorChange).collect()
    }

    pub fn translated(&self, dx: i32, dy: i32) -> Pattern {
        let mut p = self.clone();
        for c in &mut p.cmds {
            c.x += dx;
            c.y += dy;
        }
        p
    }

    /// Moved so the bounding box is centred on the origin.
    pub fn centered(&self) -> Pattern {
        let (x0, y0, x1, y1) = self.bounds();
        self.translated(-(x0 + x1) / 2, -(y0 + y1) / 2)
    }

    /// Split moves so no step exceeds the limits on either axis. A stitch
    /// that is too long travels by jumps and sews only its last step (as
    /// pyembroidery does); a long jump becomes several jumps.
    pub fn with_max_step(&self, max_stitch: i32, max_jump: i32) -> Pattern {
        let mut out = Pattern {
            name: self.name.clone(),
            threads: self.threads.clone(),
            cmds: Vec::new(),
        };
        let (mut px, mut py) = (0, 0);
        for c in &self.cmds {
            let max = match c.op {
                Op::Stitch => max_stitch,
                Op::Jump => max_jump,
                _ => {
                    out.cmds.push(*c);
                    continue;
                }
            };
            let (dx, dy) = (c.x - px, c.y - py);
            // Jump to within one step of the target, then make the move itself.
            let jn = (dx.abs().max(dy.abs()) - max + max_jump - 1).max(0) / max_jump;
            for i in 1..=jn {
                let f = |d: i32| {
                    let near = d - d.signum() * (d.abs().min(max));
                    (near as f64 * i as f64 / jn as f64).round() as i32
                };
                out.push(Op::Jump, px + f(dx), py + f(dy));
            }
            out.cmds.push(*c);
            (px, py) = (c.x, c.y);
        }
        out
    }

    /// Retarget each run of jumps to land on the stitch that follows it.
    pub fn jumps_land_on_stitches(&self) -> Pattern {
        let mut p = self.clone();
        for i in (0..p.cmds.len()).rev() {
            if p.cmds[i].op != Op::Jump {
                continue;
            }
            let next = p.cmds[i + 1..].iter().find(|c| c.op != Op::Jump);
            if let Some(&Cmd {
                op: Op::Stitch,
                x,
                y,
            }) = next
            {
                (p.cmds[i].x, p.cmds[i].y) = (x, y);
            }
        }
        p
    }

    /// Commands with their delta from the previous position (starting at 0,0).
    pub fn deltas(&self) -> impl Iterator<Item = (Cmd, i32, i32)> + '_ {
        let mut prev = (0, 0);
        self.cmds.iter().map(move |c| {
            let d = (c.x - prev.0, c.y - prev.1);
            prev = (c.x, c.y);
            (*c, d.0, d.1)
        })
    }

    /// Read a design from a Viking card. The card's long-form moves are
    /// stitches, as the app writes them. Card y is down, as in the files.
    pub fn from_card_design(d: &Design) -> Pattern {
        let mut p = Pattern::new(d.name_str());
        for (i, b) in d.blocks.iter().enumerate() {
            if i > 0 {
                p.color_change();
            }
            for s in &b.stitches {
                p.stitch(s.dx as i32, s.dy as i32);
            }
            p.threads.push(viking_thread(b.color));
        }
        p.end();
        p
    }

    /// Encode as a Viking card design, the way the Windows app does:
    /// centred on the origin, jumps folded into the next stitch (long moves
    /// use the card's long form), trims dropped. Colour codes are assigned
    /// in order because their mapping to real threads is not yet known.
    pub fn to_card_design(&self, slot: usize) -> Result<Design, String> {
        let p = self.centered();
        let mut blocks = Vec::new();
        let (mut lx, mut ly) = (0i32, 0i32);
        let mut bbox: Option<(i16, i16, i16, i16)> = None;
        for cmds in p.color_blocks() {
            let start = (lx, ly);
            let mut stitches = Vec::new();
            for c in cmds.iter().filter(|c| c.op == Op::Stitch) {
                let (dx, dy) = (c.x - lx, c.y - ly);
                let fits = |v: i32| {
                    i16::try_from(v)
                        .map_err(|_| format!("move of {v} units is too long for a card"))
                };
                stitches.push(Stitch {
                    dx: fits(dx)?,
                    dy: fits(dy)?,
                });
                (lx, ly) = (c.x, c.y);
                let (x, y) = (fits(lx)?, fits(ly)?);
                bbox = Some(match bbox {
                    None => (x, y, x, y),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                });
            }
            if stitches.is_empty() {
                continue;
            }
            let color = card::COLOR_CODES[blocks.len() % card::COLOR_CODES.len()];
            blocks.push(ColorBlock {
                color,
                start: (start.0 as i16, start.1 as i16),
                stitches,
            });
        }
        if blocks.len() > u8::MAX as usize {
            return Err(format!(
                "{} colour blocks; a card design holds at most 255",
                blocks.len()
            ));
        }
        let bbox = bbox.ok_or("design has no stitches")?;
        let mut name = [b' '; 16];
        name[..2].copy_from_slice(format!("{:02}", slot % 100).as_bytes());
        Ok(Design { name, bbox, blocks })
    }
}

/// Placeholder thread for a card colour code (the code → colour map is unknown).
fn viking_thread(code: u8) -> Thread {
    let i = card::COLOR_CODES
        .iter()
        .position(|&c| c == code)
        .unwrap_or(0);
    Thread {
        rgb: FALLBACK[i % FALLBACK.len()],
        name: format!("Viking colour {code:#04x}"),
        ..Default::default()
    }
}

/// Index of the nearest chart colour (red-mean metric), skipping index 0
/// and empty entries. Ties go to the later entry, as in pyembroidery.
pub fn nearest_color(rgb: u32, chart: &[u32]) -> usize {
    let (r, g, b) = (
        (rgb >> 16) as i64 & 0xFF,
        (rgb >> 8) as i64 & 0xFF,
        rgb as i64 & 0xFF,
    );
    let mut best = (i64::MAX, 1);
    for (i, &c) in chart.iter().enumerate().skip(1) {
        let (r2, g2, b2) = (
            (c >> 16) as i64 & 0xFF,
            (c >> 8) as i64 & 0xFF,
            c as i64 & 0xFF,
        );
        let mean = ((r + r2) as f64 / 2.0).round() as i64;
        let (dr, dg, db) = (r - r2, g - g2, b - b2);
        let dist = (((512 + mean) * dr * dr) >> 8) + 4 * dg * dg + (((767 - mean) * db * db) >> 8);
        if dist <= best.0 {
            best = (dist, i);
        }
    }
    best.1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Pattern {
        let mut p = Pattern::new("t");
        p.stitch_to(10, 10);
        p.stitch(5, -3);
        p.jump(300, 0);
        p.color_change();
        p.stitch(-2, 200);
        p.end();
        p
    }

    #[test]
    fn split_long_steps() {
        let p = sample().with_max_step(121, 121);
        for (_, dx, dy) in p.deltas() {
            assert!(dx.abs() <= 121 && dy.abs() <= 121);
        }
        assert_eq!(p.position(), sample().position());
        assert_eq!(p.color_changes(), 1);
    }

    #[test]
    fn card_roundtrip_folds_jumps() {
        let p = sample();
        let d = p.to_card_design(1).unwrap();
        assert_eq!(d.blocks.len(), 2);
        assert_eq!(d.name_str(), "01");
        let back = Pattern::from_card_design(&d);
        // Same stitch positions after centring; the jump became part of a stitch.
        let want: Vec<_> = p
            .centered()
            .cmds
            .iter()
            .filter(|c| c.op == Op::Stitch)
            .map(|c| (c.x, c.y))
            .collect();
        let got: Vec<_> = back
            .cmds
            .iter()
            .filter(|c| c.op == Op::Stitch)
            .map(|c| (c.x, c.y))
            .collect();
        assert_eq!(got, want);
        assert_eq!(back.color_changes(), 1);
    }

    #[test]
    fn card_y_is_down() {
        // Sewing showed text mirrored when card y was read as up.
        let mut p = Pattern::new("t");
        p.stitch_to(0, 0);
        p.stitch(0, 100);
        p.end();
        let d = p.to_card_design(1).unwrap();
        assert_eq!(d.blocks[0].stitches.last().unwrap().dy, 100);
        assert_eq!((d.bbox.1, d.bbox.3), (-50, 50));
    }

    #[test]
    fn nearest_color_picks_exact_match() {
        let chart = [0, 0xFF0000, 0x00FF00, 0x0000FF];
        assert_eq!(nearest_color(0x00FE01, &chart), 2);
        assert_eq!(nearest_color(0x0000FF, &chart), 3);
    }
}
