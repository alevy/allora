use crate::utils::*;
use core::ptr::{read_volatile, write_volatile};

use super::{VirtIODevice, VirtIORegs, VirtQDesc, VirtQUsed, VirtqAvailable};

pub struct VirtIOEntropy<'a> {
    regs: &'a mut VirtIORegs,
    desc: &'a mut [VirtQDesc],
    avail: &'a mut VirtqAvailable,
    used: &'a mut VirtQUsed,
    irq: crate::gic::GIC,
}

impl<'a> VirtIODevice<'a> for VirtIOEntropy<'a> {
    unsafe fn new(
        regs: &'a mut VirtIORegs,
        desc: &'a mut [VirtQDesc],
        avail: &'a mut VirtqAvailable,
        used: &'a mut VirtQUsed,
        irq: crate::gic::GIC,
    ) -> Self {
        VirtIOEntropy {
            regs,
            desc,
            avail,
            used,
            irq,
        }
    }
}

impl<'a> VirtIOEntropy<'a> {
    pub fn read(&mut self, data: &mut [u8]) {
        unsafe {
            write_volatile(
                &mut self.desc[0],
                VirtQDesc {
                    addr: (data.as_ptr() as *const _ as u64).into(),
                    len: (data.len() as u32).into(),
                    flags: (2).into(),
                    next: 0.into(),
                },
            );

            write_volatile(
                &mut self.avail.ring[self.avail.idx.native() as usize],
                0.into(),
            );
            mb();
            write_volatile(&mut self.avail.idx, (self.avail.idx.native() + 1).into());
            mb();
            write_volatile(&mut self.regs.queue_notify, 0.into());
            mb();
            self.irq.enable();
            while read_volatile(&self.used.idx).native() != read_volatile(&self.avail.idx).native()
            {
                asm!("wfi");
            }
            self.irq.disable();
        }
    }
}
