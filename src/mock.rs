//! In-memory box emulator for tests: ASCII dialect, one NOR flash card.

use std::collections::VecDeque;
use std::time::Duration;

use crate::client::{Port, Result};
use crate::frame::PAGE;

pub struct MockBox {
    pub flash: Vec<u8>,
    pub id: [u8; 2],
    pub card_present: bool,
    /// Drop the next command, as the real box does right after open.
    pub drop_next: bool,
    pub erase_count: usize,
    pub pages_written: usize,
    pending: Vec<u8>,
    out: VecDeque<u8>,
}

impl MockBox {
    /// A 1 MiB Am29F080 card, blank.
    pub fn new() -> Self {
        MockBox {
            flash: vec![0xFF; 0x10_0000],
            id: [0x01, 0xD5],
            card_present: true,
            drop_next: true,
            erase_count: 0,
            pages_written: 0,
            pending: Vec::new(),
            out: VecDeque::new(),
        }
    }

    fn reply(&mut self, parts: &[&[u8]]) {
        for p in parts {
            self.out.extend(p.iter());
        }
    }

    /// Consume one complete command from `pending`, if there is one.
    fn step(&mut self) -> bool {
        let p = &self.pending;
        let need = match p.get(..3) {
            Some(b"wr1") => 3 + 4 + PAGE + 2,
            Some(b"rd1") => 3 + 4 + 2 + 2,
            Some(_) => match p.iter().position(|&b| b == b'\n') {
                Some(i) => i + 1,
                None => return false,
            },
            None => return false,
        };
        if p.len() < need {
            return false;
        }
        let cmd: Vec<u8> = self.pending.drain(..need).collect();
        if std::mem::take(&mut self.drop_next) {
            return true;
        }
        let addr = |c: &[u8]| u32::from_le_bytes(c[3..7].try_into().unwrap()) as usize;
        match &cmd[..3] {
            b"vr1" => self.reply(&[b"v10\r\n>"]),
            b"ld0" | b"ld1" => self.reply(&[&cmd[..3], b"\r\n>"]),
            b"cd1" => {
                let s = if self.card_present { b"1" } else { b"0" };
                self.reply(&[b"cd1", s, b"\r\n>"]);
            }
            b"id1" => {
                let id = self.id;
                self.reply(&[b"id1", &id, b"\r\n>"]);
            }
            b"er1" => {
                self.flash.fill(0xFF);
                self.erase_count += 1;
                self.reply(&[b"er11\r\n>"]);
            }
            b"rd1" => {
                let (a, n) = (addr(&cmd), u16::from_le_bytes([cmd[7], cmd[8]]) as usize);
                let data = self.flash[a..a + n].to_vec();
                self.reply(&[&cmd[..9], &data, b"\r\n>"]);
            }
            b"wr1" => {
                let a = addr(&cmd);
                // NOR programming can only clear bits.
                for (dst, src) in self.flash[a..a + PAGE].iter_mut().zip(&cmd[7..7 + PAGE]) {
                    *dst &= src;
                }
                self.pages_written += 1;
                self.reply(&[b"wr11\r\n>"]);
            }
            _ => self.reply(&[b"cmder\r\n>"]),
        }
        true
    }
}

impl Default for MockBox {
    fn default() -> Self {
        Self::new()
    }
}

impl Port for MockBox {
    fn send(&mut self, data: &[u8]) -> Result<()> {
        self.pending.extend_from_slice(data);
        while self.step() {}
        Ok(())
    }

    fn recv(&mut self, n: usize, _timeout: Duration) -> Result<Vec<u8>> {
        let n = n.min(self.out.len());
        Ok(self.out.drain(..n).collect())
    }

    fn purge_rx(&mut self) -> Result<()> {
        self.out.clear();
        Ok(())
    }
}
