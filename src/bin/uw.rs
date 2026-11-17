//! `vikant` — command-line access to a Vikant Ultimate Box.

use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use ultiwriter::card::{Card, CardError, IMAGE_CAPACITY};
use ultiwriter::formats::{self, Format};
use ultiwriter::pattern::{Op, Pattern};
use ultiwriter::{CardKind, Client, FtdiPort, Result};

type CliResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

const USAGE: &str = "\
usage: vikant <command>
  info                         box dialect, card presence, flash ID/size
  read <addr> <len>            hexdump bytes from the card
  dump <file>                  read the whole card to a file
  ls [file]                    list Viking designs on the card (or in a dump)
  write <addr> <file> --yes    program bytes (target must be erased/blank)
  erase --yes                  chip-erase the card
  restore <file> --yes         erase, write the image, verify

embroidery files (pes pec dst exp jef vp3 pen; format from the extension):
  show <file> [--cmds]         summarise a design (--cmds: every command)
  convert <in> <out>           convert between formats
  export <dir> [fmt] [--from dump.bin]
                               save each card design as <dir>/NN.<fmt> (default vp3)
  image <out.bin> <file>...    build a Viking card image from design files
  import <file>... --yes       erase the card and write these designs to it
Addresses/lengths accept decimal or 0x-hex.";

fn num(s: &str) -> std::result::Result<u32, String> {
    let r = match s.strip_prefix("0x") {
        Some(h) => u32::from_str_radix(h, 16),
        None => s.parse(),
    };
    r.map_err(|_| format!("bad number: {s}"))
}

fn progress(label: &str, total: usize) -> impl FnMut(usize) {
    let start = Instant::now();
    let label = label.to_string();
    let mut last_pct = usize::MAX;
    move |done| {
        let pct = done * 100 / total.max(1);
        if pct == last_pct {
            return;
        }
        last_pct = pct;
        eprint!(
            "\r{label}: {pct:3}% ({:.1}s)",
            start.elapsed().as_secs_f32()
        );
        if done == total {
            eprintln!();
        }
        let _ = std::io::stderr().flush();
    }
}

fn connect() -> Result<Client<FtdiPort>> {
    Client::connect(FtdiPort::open()?)
}

fn card_size(c: &mut Client<FtdiPort>) -> Result<usize> {
    if !c.card_present()? {
        return Err(ultiwriter::Error::NoCard);
    }
    Ok(c.identify()?.size() as usize)
}

fn hexdump(base: u32, data: &[u8]) {
    for (i, row) in data.chunks(16).enumerate() {
        let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
        let txt: String = row
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!(
            "{:08x}  {:<47}  {txt}",
            base as usize + i * 16,
            hex.join(" ")
        );
    }
}

/// Read just enough of the card to parse its Viking directory.
fn read_image(c: &mut Client<FtdiPort>) -> CliResult<Vec<u8>> {
    let mut img = Vec::new();
    let mut want = 4096;
    loop {
        let at = img.len();
        img.resize(want, 0);
        c.read(CardKind::Viking, at as u32, &mut img[at..])?;
        match Card::parse_len(&img) {
            Err(CardError::Truncated { need }) if need <= IMAGE_CAPACITY => {
                want = need.next_multiple_of(4096).max(want * 2)
            }
            _ => return Ok(img),
        }
    }
}

fn list(img: &[u8]) -> CliResult {
    let (card, len) = Card::parse_len(img)?;
    println!("{} design(s), image {len} bytes", card.designs.len());
    for (i, d) in card.designs.iter().enumerate() {
        let (x0, y0, x1, y1) = d.bbox;
        let colors: Vec<String> = d
            .blocks
            .iter()
            .map(|b| format!("{:#04x}", b.color))
            .collect();
        println!(
            "  #{:<3} {:<16} {:>6} stitches  {:>5.1} x {:<5.1} mm  colours [{}]",
            i + 1,
            d.name_str(),
            d.stitch_count(),
            (x1 - x0) as f32 / 10.0,
            (y1 - y0) as f32 / 10.0,
            colors.join(" ")
        );
    }
    Ok(())
}

fn summary(p: &Pattern) {
    let (x0, y0, x1, y1) = p.bounds();
    let count = |op| p.cmds.iter().filter(|c| c.op == op).count();
    println!("name:     {}", p.name);
    println!(
        "size:     {:.1} x {:.1} mm",
        (x1 - x0) as f32 / 10.0,
        (y1 - y0) as f32 / 10.0
    );
    println!(
        "commands: {} stitches, {} jumps, {} trims, {} colours",
        count(Op::Stitch),
        count(Op::Jump),
        count(Op::Trim),
        p.color_count()
    );
    for i in 0..p.color_count() {
        let t = p.thread(i);
        let extra = [t.name.as_str(), t.catalog.as_str(), t.brand.as_str()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        println!("  colour {:<3} {}  {extra}", i + 1, t.hex());
    }
}

/// Designs → card image, as the Windows app lays them out.
fn build_image(files: &[&str]) -> CliResult<Vec<u8>> {
    let mut card = Card::default();
    for (i, f) in files.iter().enumerate() {
        let p = formats::read_file(Path::new(f))?;
        let d = p.to_card_design(i + 1).map_err(|e| format!("{f}: {e}"))?;
        println!(
            "  #{:<3} {f}: {} stitches, {} colours",
            i + 1,
            d.stitch_count(),
            d.blocks.len()
        );
        card.designs.push(d);
    }
    let img = card.to_image()?;
    // The image must read back as exactly what we built.
    if Card::parse(&img)? != card {
        return Err("internal error: card image does not round-trip".into());
    }
    Ok(img)
}

/// Erase the card, write `image` (skipping blank pages) and verify it.
fn flash_image(c: &mut Client<FtdiPort>, image: &[u8]) -> CliResult {
    let size = card_size(c)?;
    if image.len() > size {
        return Err(format!("image is {} bytes, card is {size}", image.len()).into());
    }
    let t = Instant::now();
    c.erase()?;
    println!("erased in {:.1}s", t.elapsed().as_secs_f32());
    c.write_pages(
        CardKind::Viking,
        0,
        image,
        true,
        progress("write", image.len()),
    )?;
    let mut back = vec![0; image.len()];
    c.read_with_progress(
        CardKind::Viking,
        0,
        &mut back,
        progress("verify", image.len()),
    )?;
    if back != image {
        return Err("verify failed".into());
    }
    Ok(())
}

fn run(args: &[String]) -> CliResult {
    let mut confirmed = false;
    let mut show_cmds = false;
    let mut from: Option<String> = None;
    let mut pos: Vec<&str> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--yes" => confirmed = true,
            "--cmds" => show_cmds = true,
            "--from" => from = Some(it.next().ok_or("--from needs a file")?.clone()),
            other => pos.push(other),
        }
    }
    let need_yes = || -> CliResult {
        if confirmed {
            Ok(())
        } else {
            Err("this modifies the card; re-run with --yes".into())
        }
    };
    match pos.as_slice() {
        ["info"] => {
            let port = FtdiPort::open()?;
            println!(
                "device:   {} (pid {:#06x})",
                port.product().unwrap_or_default(),
                port.product_id
            );
            let mut c = Client::connect(port)?;
            println!("dialect:  {:?}", c.dialect());
            let present = c.card_present()?;
            println!("card:     {}", if present { "present" } else { "none" });
            if present {
                let id = c.identify()?;
                println!(
                    "flash:    {} {:#04x}/{:#04x}, {} KiB",
                    id.manufacturer_name(),
                    id.manufacturer,
                    id.device,
                    id.size() / 1024
                );
            }
        }
        ["read", addr, len] => {
            let (addr, len) = (num(addr)?, num(len)? as usize);
            let mut c = connect()?;
            let mut buf = vec![0; len];
            c.read(CardKind::Viking, addr, &mut buf)?;
            hexdump(addr, &buf);
        }
        ["dump", file] => {
            let mut c = connect()?;
            let size = card_size(&mut c)?;
            let mut buf = vec![0; size];
            c.read_with_progress(CardKind::Viking, 0, &mut buf, progress("read", size))?;
            std::fs::write(file, &buf)?;
            println!("wrote {size} bytes to {file}");
        }
        ["ls", file] => list(&std::fs::read(file)?)?,
        ["ls"] => {
            let mut c = connect()?;
            card_size(&mut c)?;
            list(&read_image(&mut c)?)?;
        }
        ["write", addr, file] => {
            need_yes()?;
            let (addr, data) = (num(addr)?, std::fs::read(file)?);
            let mut c = connect()?;
            let size = card_size(&mut c)?;
            if addr as usize + data.len() > size {
                return Err(format!("write past end of {size}-byte card").into());
            }
            c.write_pages(
                CardKind::Viking,
                addr,
                &data,
                false,
                progress("write", data.len()),
            )?;
            let mut back = vec![0; data.len()];
            c.read(CardKind::Viking, addr, &mut back)?;
            let padded_ok = back == data;
            println!(
                "{}",
                if padded_ok {
                    "verified"
                } else {
                    "VERIFY FAILED (was the target erased?)"
                }
            );
            if !padded_ok {
                return Err("verify failed".into());
            }
        }
        ["erase"] => {
            need_yes()?;
            let mut c = connect()?;
            card_size(&mut c)?;
            let t = Instant::now();
            c.erase()?;
            println!("erased in {:.1}s", t.elapsed().as_secs_f32());
        }
        ["restore", file] => {
            need_yes()?;
            let image = std::fs::read(file)?;
            flash_image(&mut connect()?, &image)?;
            println!("restored and verified");
        }
        ["show", file] => {
            let p = formats::read_file(Path::new(file))?;
            summary(&p);
            if show_cmds {
                for c in &p.cmds {
                    println!("{:?} {} {}", c.op, c.x, c.y);
                }
            }
        }
        ["convert", src, dst] => {
            let p = formats::read_file(Path::new(src))?;
            formats::write_file(Path::new(dst), &p)?;
            println!(
                "{src} -> {dst}: {} stitches, {} colours",
                p.stitch_count(),
                p.color_count()
            );
        }
        ["export", dir, rest @ ..] => {
            let fmt = match rest {
                [] => Format::Vp3,
                [ext] => Format::from_ext(ext).ok_or_else(|| format!("unknown format: {ext}"))?,
                _ => return Err(USAGE.into()),
            };
            let img = match &from {
                Some(f) => std::fs::read(f)?,
                None => {
                    let mut c = connect()?;
                    card_size(&mut c)?;
                    read_image(&mut c)?
                }
            };
            let card = Card::parse(&img)?;
            std::fs::create_dir_all(dir)?;
            for (i, d) in card.designs.iter().enumerate() {
                let path = Path::new(dir).join(format!("{:02}.{}", i + 1, fmt.ext()));
                std::fs::write(&path, fmt.write(&Pattern::from_card_design(d))?)?;
                println!("{}: {} stitches", path.display(), d.stitch_count());
            }
        }
        ["image", out, files @ ..] if !files.is_empty() => {
            let img = build_image(files)?;
            std::fs::write(out, &img)?;
            println!("wrote {} byte card image to {out}", img.len());
        }
        ["import", files @ ..] if !files.is_empty() => {
            need_yes()?;
            let img = build_image(files)?;
            flash_image(&mut connect()?, &img)?;
            println!("wrote {} design(s) and verified", files.len());
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
