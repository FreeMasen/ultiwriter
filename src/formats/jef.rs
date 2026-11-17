//! Janome JEF: 116-byte header + palette, then 2-byte deltas (y up).

use super::palette::{JEF_THREADS, rgbs};
use super::{FormatError, Reader, err};
use crate::pattern::{Op, Pattern, Thread, nearest_color};

pub const MAX_STEP: i32 = 127;
const HEADER: u32 = 0x74;

/// Hoop codes, as pyembroidery picks them from the design size (units).
fn hoop(w: i32, h: i32) -> u32 {
    match (w, h) {
        (w, h) if w < 500 && h < 500 => 1,   // 50 x 50
        (w, h) if w < 1260 && h < 1100 => 3, // 126 x 110
        (w, h) if w < 1400 && h < 2000 => 2, // 140 x 200
        (w, h) if w < 2000 && h < 2000 => 4, // 200 x 200
        _ => 0,                              // 110 x 110
    }
}

/// Local-time-free "YYYYMMDDHHMMSS" (UTC) for the header.
fn timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!(
        "{y:04}{m:02}{d:02}{:02}{:02}{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

pub fn write(p: &Pattern) -> Vec<u8> {
    write_dated(p, &timestamp())
}

pub fn write_dated(p: &Pattern, date: &str) -> Vec<u8> {
    let p = p.with_max_step(MAX_STEP, MAX_STEP);
    let chart = rgbs(JEF_THREADS);
    let colors = p.color_count() as u32;
    let palette: Vec<u32> = (0..colors as usize)
        .map(|i| nearest_color(p.thread(i).rgb, &chart) as u32)
        .collect();
    let points = 1 + p
        .cmds
        .iter()
        .map(|c| match c.op {
            Op::Stitch => 1,
            Op::Jump | Op::ColorChange => 2,
            _ => 0,
        })
        .sum::<u32>();
    let (x0, y0, x1, y1) = p.bounds();
    let (w, h) = (x1 - x0, y1 - y0);
    let (hw, hh) = (
        (w as f64 / 2.0).round() as i32,
        (h as f64 / 2.0).round() as i32,
    );

    let mut out = Vec::new();
    let le = |out: &mut Vec<u8>, v: i32| out.extend(v.to_le_bytes());
    le(&mut out, (HEADER + colors * 8) as i32);
    le(&mut out, 0x14);
    out.extend(format!("{date:.14}").as_bytes());
    out.extend([0, 0]);
    le(&mut out, colors as i32);
    le(&mut out, points as i32);
    le(&mut out, hoop(w, h) as i32);
    for v in [hw, hh, hw, hh] {
        le(&mut out, v);
    }
    // Distance to each hoop edge: 110x110, 50x50, 140x200, 140x200 (half sizes).
    for (hx, hy) in [(550, 550), (250, 250), (700, 1000), (700, 1000)] {
        let (ex, ey) = (hx - hw, hy - hh);
        let edge = if ex.min(ey) >= 0 {
            [ex, ey, ex, ey]
        } else {
            [-1; 4]
        };
        for v in edge {
            le(&mut out, v);
        }
    }
    for &c in &palette {
        le(&mut out, c as i32);
    }
    for _ in 0..colors {
        le(&mut out, 0x0D);
    }
    debug_assert_eq!(out.len() as u32, HEADER + colors * 8);
    for (c, dx, dy) in p.deltas() {
        let d = [dx as i8 as u8, (-dy) as i8 as u8];
        match c.op {
            Op::Stitch => out.extend(d),
            Op::ColorChange => out.extend([0x80, 0x01, d[0], d[1]]),
            Op::Jump => out.extend([0x80, 0x02, d[0], d[1]]),
            Op::Trim => {}
            Op::End => break,
        }
    }
    out.extend([0x80, 0x10]);
    out
}

pub fn read(data: &[u8]) -> Result<Pattern, FormatError> {
    let mut r = Reader::new(data);
    let stitch_offset = r.u32le()? as usize;
    r.skip(20)?;
    let colors = r.u32le()? as usize;
    r.skip(88)?;
    if colors > 1000 || stitch_offset > data.len() {
        return err("not a JEF file (implausible header)");
    }
    let mut p = Pattern::default();
    for _ in 0..colors {
        let i = (r.u32le()? as i32).unsigned_abs() as usize;
        let t = &JEF_THREADS[i % JEF_THREADS.len()];
        p.threads.push(Thread {
            rgb: t.rgb,
            name: t.name.into(),
            catalog: t.code.into(),
            brand: "Janome".into(),
        });
    }
    let mut recs = data[stitch_offset..].as_chunks::<2>().0.iter();
    let d = |b: &[u8]| (b[0] as i8 as i32, -(b[1] as i8 as i32));
    while let Some(b) = recs.next() {
        if b[0] != 0x80 {
            let (dx, dy) = d(b);
            p.stitch(dx, dy);
            continue;
        }
        if b[1] == 0x10 {
            break;
        }
        let Some(arg) = recs.next() else { break };
        let (dx, dy) = d(arg);
        match b[1] {
            0x01 => {
                p.color_change();
                if dx != 0 || dy != 0 {
                    p.jump(dx, dy);
                }
            }
            0x02 => p.jump(dx, dy),
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
    fn roundtrip_and_palette() {
        let p = sample();
        let bytes = write_dated(&p, "20261112120000");
        assert_eq!(&bytes[8..22], b"20261112120000");
        let back = read(&bytes).unwrap();
        assert_eq!(
            stitches(&back),
            stitches(&p.with_max_step(MAX_STEP, MAX_STEP))
        );
        assert_eq!(back.threads.len(), 2);
        assert_eq!(back.color_changes(), 1);
    }

    #[test]
    fn timestamp_shape() {
        let t = timestamp();
        assert_eq!(t.len(), 14);
        assert!(t.starts_with("20"));
    }
}
