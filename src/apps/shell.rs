use core::fmt::Write;
use core::str::from_utf8;

use crate::uart::UART;
use crate::virtio::{VirtIOBlk, VirtIOEntropy, VirtIONet };

pub struct Shell<'b> {
    pub blk: VirtIOBlk<'b>,
    pub entropy: VirtIOEntropy<'b>,
    pub net: Option<VirtIONet<'b>>,
}

impl<'b> Shell<'b> {
    fn get_random<F: FnMut(&[u8])>(&mut self, mut f: F) {
        let mut data: [u8; 16] = [0; 16];
        self.entropy.read(&mut data);
        f(b"Random: ");
        f(&data);
    }

    fn write_random<F: FnMut(&[u8])>(&mut self, words: &mut dyn Iterator<Item = &[u8]>, mut f: F) {
        let mut sector = words
            .next()
            .and_then(|sec| from_utf8(sec).ok())
            .and_then(|sec| sec.parse::<u64>().ok())
            .unwrap_or(0);
        let mut len = words
            .next()
            .and_then(|len| from_utf8(len).ok())
            .and_then(|len| len.parse::<usize>().ok())
            .unwrap_or(0);
        while len > 0 {
            let mut outdata: [u8; 512] = [0; 512];
            let curlen = core::cmp::min(512, len);
            {
                let curbuf = &mut outdata[..curlen];
                self.entropy.read(curbuf);
                for b in curbuf.iter_mut() {
                    *b = ((*b as u32 * 100) / 272 + 32) as u8;
                }
            }
            self.blk.write(sector, &outdata);
            sector += 1;
            len -= curlen;
        }
        f(b"done");
    }

    fn read<F: FnMut(&[u8])>(&mut self, words: &mut dyn Iterator<Item = &[u8]>, mut f: F) {
        let sector = words
            .next()
            .and_then(|sec| from_utf8(sec).ok())
            .and_then(|sec| sec.parse::<u64>().ok())
            .unwrap_or(0);
        let mut len = words
            .next()
            .and_then(|len| from_utf8(len).ok())
            .and_then(|len| len.parse::<usize>().ok())
            .unwrap_or(512);
        let mut data: [u8; 512] = [0; 512];
        loop {
            self.blk.read(sector, &mut data);
            if len > 512 {
                f(&data);
                len -= 512;
            } else {
                f(&data[..len]);
                break;
            }
        }
    }

    /*fn write<F: FnMut(&[u8])>(&mut self, words: &mut dyn Iterator<Item = &[u8]>, mut f: F) {
        let mut sector = words
            .next()
            .and_then(|sec| from_utf8(sec).ok())
            .and_then(|sec| sec.parse::<u64>().ok())
            .unwrap_or(0);
        let mut len = words
            .next()
            .and_then(|len| from_utf8(len).ok())
            .and_then(|len| len.parse::<usize>().ok())
            .unwrap_or(0);
        while len > 0 {
            let mut outdata: [u8; 512] = [0; 512];
            let curlen = core::cmp::min(512, len);
            {
                let curbuf = &mut outdata[..curlen];
                for b in curbuf.iter_mut() {
                    *b = self.uart.read_byte();
                    if *b == b'\r' {
                        *b = b'\n';
                    }
                    self.uart.write_byte(*b);
                }
            }
            self.blk.write(sector, &outdata);
            sector += 1;
            len -= curlen;
        }
    }*/

    pub fn do_line<F>(&mut self, line: &[u8], mut f: F) -> bool where F: FnMut(&[u8]) {
        let line = line.split(|c| *c == b'\n' || *c == b'\r').next().unwrap_or(&[]);
        let mut words = line.split(|c| *c == b' ');
        match words.next() {
            Some(b"rand") => {
                self.get_random(f);
            }
            Some(b"writerand") => {
                self.write_random(&mut words, f);
            }
            Some(b"read") => {
                self.read(&mut words, f);
            }
            /*Some(b"write") => {
                self.write(&mut words, f);
            }*/
            Some(b"netshell") => {
                self.net.take().as_mut().map(|vnet| {
                    let mut net = super::net::Net {
                        net: vnet,
                    };
                    net.run(self);
                });
            }
            Some(b"exit") => {
                return true;
            }
            _ => {
                f(b"Unknown command \"");
                f(line);
                f(b"\"");
            }
        }
        return false;
    }
}

pub fn main(uart: &mut UART, app: &mut Shell) {
    loop {
        let _ = write!(uart, "$> ");
        let mut buf = [0; 1024];
        let line = uart.read_line(&mut buf, true);
        if app.do_line(line, |output| {
            uart.write_bytes(output);
        }) {
            break;
        }
        uart.write_byte(b'\n');
    }
}
