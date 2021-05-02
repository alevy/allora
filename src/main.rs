#![no_main]
#![no_std]
#![feature(asm, global_asm)]

pub mod device_tree;
pub mod utils;
pub mod virtio;
pub mod uart;

mod apps;

use virtio::{VirtIORegs, VirtIODevice};

#[cfg(target_arch = "aarch64")]
global_asm!(include_str!("boot.S"));

use core::panic::PanicInfo;
use core::fmt::Write;

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
        let size_cell = root
            .prop_by_name("#size-cells")
            .map(|sc| {
                let mut buf = [0; 4];
                buf.copy_from_slice(sc.value);
                u32::from_be_bytes(buf) as usize
            })
            .unwrap_or(2);
        let address_cell = root
            .prop_by_name("#address-cells")
            .map(|sc| {
                let mut buf = [0; 4];
                buf.copy_from_slice(sc.value);
                u32::from_be_bytes(buf) as usize
            })
            .unwrap_or(2);

        if let Some(chosen) = root.child_by_name("chosen") {
            chosen
                .prop_by_name("stdout-path")
                .map(|stdout_path| null_terminated_str(stdout_path.value))
                .filter(|stdout_path| stdout_path == b"/pl011@9000000")
                .map(|stdout_path| {
                    root.child_by_path(stdout_path).map(|stdout| {
                        if let Some(reg) = stdout.prop_by_name("reg") {
                            let (addr, rest) = regs_to_usize(reg.value, address_cell);
                            let (size, _) = regs_to_usize(rest, size_cell);
                            if size == 0x1000 {
                                uart = Some(unsafe { uart::UART::new(addr as _) });
                            }
                        }
                    });
                });
        }
        uart.as_mut().map(|uart| {
            let _ = write!(uart, "We booted!\n");

            let mut virtio_blk = None;
            let mut blk_desc = [virtio::VirtQDesc::empty(); 128];
            let mut blk_avail = virtio::VirtqAvailable::empty();
            let mut blk_used = virtio::VirtQUsed::empty();

            let mut virtio_entropy: Option<virtio::VirtIOEntropy> = None;
            let mut entropy_desc = [virtio::VirtQDesc::empty(); 128];
            let mut entropy_avail = virtio::VirtqAvailable::empty();
            let mut entropy_used = virtio::VirtQUsed::empty();

            for child in root.children_by_prop("compatible", |prop| prop.value == b"virtio,mmio\0")
            {
                if let Some(reg) = child.prop_by_name("reg") {
                    let (addr, _rest) = regs_to_usize(reg.value, address_cell);
                    if let Some(virtio) = unsafe { VirtIORegs::new(addr as *mut VirtIORegs) } {
                        match virtio.device_id() {
                            virtio::DeviceId::Blk => {
                                virtio_blk = virtio::VirtIOBlk::init(virtio, &mut blk_desc, &mut blk_avail, &mut blk_used);
                            },
                            virtio::DeviceId::Entropy => {
                                virtio_entropy = virtio::VirtIOEntropy::init(virtio, &mut entropy_desc, &mut entropy_avail, &mut entropy_used);
                            },
                            _ => {},
                        }
                    }
                }
            }
            virtio_blk.map(|blk| {
                virtio_entropy.map(|entropy| {
                    let mut shell = apps::shell::App {
                        uart,
                        blk,
                        entropy,
                    };
                    shell.main();
                });
            });
        });
    }
}

#[panic_handler]
fn panic(panic_info: &PanicInfo<'_>) -> ! {
    let mut uart = unsafe { uart::UART::new(0x0900_0000 as _) };
    let _ = write!(uart, "Panic occurred: {}\n", panic_info);
    loop {}
}
