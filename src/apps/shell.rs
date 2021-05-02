use core::fmt::Write;
use core::str::from_utf8;

use crate::uart::UART;
use crate::virtio::VirtIOBlk;
use crate::virtio::VirtIOEntropy;

pub struct App<'a, 'b> {
    pub uart: &'a mut UART,
    pub blk: VirtIOBlk<'b>,
    pub entropy: VirtIOEntropy<'b>,
}

impl<'a, 'b> App<'a, 'b> {
    fn get_random(&mut self) {
        let mut data: [u8; 16] = [0; 16];
        self.entropy.read(&mut data);
        let _ = write!(self.uart, "Random: {:?}\n", &data);
    }

    fn write_random(&mut self, words: &mut dyn Iterator<Item=&[u8]>) {
        let mut sector = words.next().and_then(|sec| from_utf8(sec).ok()).and_then(|sec| sec.parse::<u64>().ok()).unwrap_or(0);
        let mut len = words.next().and_then(|len| from_utf8(len).ok()).and_then(|len| len.parse::<usize>().ok()).unwrap_or(0);
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
    }

    fn read(&mut self, words: &mut dyn Iterator<Item=&[u8]>) {
        let sector = words.next().and_then(|sec| from_utf8(sec).ok()).and_then(|sec| sec.parse::<u64>().ok()).unwrap_or(0);
        let mut len = words.next().and_then(|len| from_utf8(len).ok()).and_then(|len| len.parse::<usize>().ok()).unwrap_or(512);
        let mut data: [u8; 512] = [0; 512];
        loop {
            self.blk.read(sector, &mut data);
            if len > 512 {
                self.uart.write_bytes(&data);
                len -= 512;
            } else {
                self.uart.write_bytes(&data[..len]);
                self.uart.write_byte(b'\n');
                break;
            }
        }
    }


    fn write(&mut self, words: &mut dyn Iterator<Item=&[u8]>) {
        let mut sector = words.next().and_then(|sec| from_utf8(sec).ok()).and_then(|sec| sec.parse::<u64>().ok()).unwrap_or(0);
        let mut len = words.next().and_then(|len| from_utf8(len).ok()).and_then(|len| len.parse::<usize>().ok()).unwrap_or(0);
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
    }


    pub fn main(&mut self) {
        loop {
            let _ = write!(self.uart, "$> ");
            let mut buf = [0; 1024];
            let line = self.uart.read_line(&mut buf, true);
            let mut words = line.split(|c| *c == b' ');
            match words.next() {
                Some(b"rand") => {
                    self.get_random();
                },
                Some(b"writerand") => {
                    self.write_random(&mut words);
                },
                Some(b"read") => {
                    self.read(&mut words);
                },
                Some(b"write") => {
                    self.write(&mut words);
                },
                Some(b"exit") => {
                    break;
                },
                _ => {
                    let _ = write!(self.uart, "Unknown command \"{}\"\n", from_utf8(line).unwrap_or("unknown"));
                }
            }
        }
    }
}

