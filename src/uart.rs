use core::fmt::Write;
use core::ptr;
use core::str;

pub struct UART(*mut u32);

impl UART {
    pub const unsafe fn new(base_addr: *mut u32) -> UART {
        UART(base_addr)
    }

    pub fn write_byte(&mut self, byte: u8) {
        unsafe {
            ptr::write_volatile(self.0, byte as u32);
            while ptr::read_volatile(self.0.offset(0x18 / 4)) & 1 << 3 != 0 {}
        }
    }

    pub fn write_bytes(&mut self, s: &[u8]) {
        for byte in s.iter() {
            self.write_byte(*byte);
        }
    }

    #[inline(never)]
    pub fn read_byte(&mut self) -> u8 {
        unsafe {
            ptr::write_volatile(self.0.offset(0x38 / 4), 1 << 4);
            while ptr::read_volatile(self.0.offset(0x18 / 4)) & (1 << 4) != 0 {}
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

