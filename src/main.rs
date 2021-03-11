#![no_main]
#![no_std]
#![feature(global_asm)]

use core::fmt::Write;
use core::ptr;
use core::str;

mod device_tree;

#[cfg(target_arch = "aarch64")]
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

fn null_terminated_str(bytes: &[u8]) -> &[u8] {
    if bytes[bytes.len() - 1] == 0 {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    }
}

fn regs_to_usize(regs: &[u8], cell_size: usize) -> (usize, &[u8]) {
    let mut result = 0;
    let (work, rest) = regs.split_at(cell_size * 4);
    for chunk in work.chunks(4) {
        let mut c = [0; 4];
        c.copy_from_slice(chunk);
        result = result << 32 | (u32::from_be_bytes(c) as usize);
    }
    (result, rest)
}

#[no_mangle]
pub extern "C" fn kernel_main(dtb: &device_tree::DeviceTree) {
    let mut uart = None;
    if let Some(root) = dtb.root() {
        let size_cell = root.prop_by_name("#size-cells").map(|sc| {
            let mut buf = [0; 4];
            buf.copy_from_slice(sc.value);
            u32::from_be_bytes(buf) as usize
        }).unwrap_or(2);
        let address_cell = root.prop_by_name("#address-cells").map(|sc| {
            let mut buf = [0; 4];
            buf.copy_from_slice(sc.value);
            u32::from_be_bytes(buf) as usize
        }).unwrap_or(2);

        if let Some(chosen) = root.child_by_name("chosen") {
            chosen.prop_by_name("stdout-path")
                .map(|stdout_path| null_terminated_str(stdout_path.value))
                .filter(|stdout_path| stdout_path == b"/pl011@9000000")
                .map(|stdout_path| {
                    root.child_by_path(stdout_path)
                        .map(|stdout| {
                            if let Some(reg) = stdout.prop_by_name("reg") {
                                let (addr, rest) = regs_to_usize(reg.value, address_cell);
                                let (size, _) = regs_to_usize(rest, size_cell);
                                if size == 0x1000 {
                                    uart = Some(UART(addr as *mut u8));
                                }
                            }
                        });
                });
        }
        uart.as_mut().map(|uart| {
            let _ = write!(uart, "We booted!\n");
        });
    }
}

#[panic_handler]
fn panic(panic_info: &PanicInfo<'_>) -> ! {
    let mut uart = UART(0x0900_0000 as *mut u8);
    let _ = uart.write_fmt(format_args!("Panic occurred: {}\n", panic_info));
    loop {}
}
