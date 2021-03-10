#![no_main]
#![no_std]
#![feature(global_asm)]

use core::fmt::Write;
use core::ptr;
use core::str;

mod device_tree;

global_asm!(include_str!("boot.S"));

use core::panic::PanicInfo;

struct UART(*mut u8);

impl core::fmt::Write for UART {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.as_bytes() {
            unsafe {
                ptr::write_volatile(self.0, *byte);
            }
        }
        Ok(())
    }
}

#[no_mangle]
pub extern "C" fn kernel_main(dtb: &device_tree::Header) {
    let mut uart = UART(0x0900_0000 as *mut u8);
    for node in unsafe { dtb.nodes() } {
        for _ in 0..node.depth {
            let _ = write!(&mut uart, "  ");
        }
        let _ = write!(&mut uart, "{}\n", node.name);
    }
    let _ = write!(&mut uart, "{}\n", dtb );
}

#[panic_handler]
fn panic(panic_info: &PanicInfo<'_>) -> ! {
    let mut uart = UART(0x0900_0000 as *mut u8);
    let _ = uart.write_fmt(format_args!("Panic occurred: {}\n", panic_info));
    loop {}
}
