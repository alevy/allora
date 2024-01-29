use core::arch::asm;
use core::fmt::Write;
use core::ptr;
use core::str;

use crate::gic::GIC;

pub struct UART(*mut u32, GIC);
unsafe impl Send for UART {}
unsafe impl Sync for UART {}

pub const IRQ: u32 = 0x21;

impl UART {
    pub const unsafe fn new(base_addr: *mut u32, irq: GIC) -> UART {
        UART(base_addr, irq)
    }

    pub fn write_byte(&mut self, byte: u8) {
        unsafe {
            ptr::write_volatile(self.0, byte as u32);
            let orig_mask = ptr::read(self.0.offset(0x38 / 4));
            ptr::write_volatile(self.0.offset(0x38 / 4), orig_mask | (1 << 3));
            self.1.enable();
            while ptr::read_volatile(self.0.offset(0x18 / 4)) & 1 << 3 != 0 {
                asm!("wfi");
                self.1.clear();
            }
            self.1.disable();
            ptr::write_volatile(self.0.offset(0x38 / 4), orig_mask);
        }
    }

    pub fn write_bytes(&mut self, s: &[u8]) {
        for byte in s.iter() {
            self.write_byte(*byte);
        }
    }

    pub fn read_byte(&mut self) -> u8 {
        unsafe {
            let orig_mask = ptr::read(self.0.offset(0x38 / 4));
            ptr::write_volatile(self.0.offset(0x38 / 4), orig_mask | (1 << 4));
            self.1.enable();
            while ptr::read_volatile(self.0.offset(0x18 / 4)) & (1 << 4) != 0 {
                asm!("wfi");
                self.1.clear();
            }
            self.1.disable();
            ptr::write_volatile(self.0.offset(0x38 / 4), orig_mask);
            ptr::read_volatile(self.0) as u8
        }
    }

    pub fn read_line<'a>(&mut self, buf: &'a mut [u8], echo: bool) -> &'a [u8] {
        let mut max_len = buf.len();
        let mut count = 0;
        while max_len > 0 {
            let cur = self.read_byte();
            if cur == b'\r' || cur == b'\n' {
                break;
            }
            if echo {
                self.write_byte(cur);
            }
            buf[count] = cur;
            count += 1;
            max_len -= 1;
        }
        if echo {
            self.write_byte(b'\n');
        }
        &buf[0..count]
    }
}

impl Write for UART {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_bytes(s.as_bytes());
        Ok(())
    }
}
