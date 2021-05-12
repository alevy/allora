use crate::utils::*;
use core::ptr::{read_volatile, write_volatile};

use super::{VirtIODevice, VirtIORegs, VirtQDesc, VirtQUsed, VirtqAvailable, LEU32, LEU64};

pub struct VirtIOBlk<'a> {
    regs: &'a mut VirtIORegs,
    desc: &'a mut [VirtQDesc],
    avail: &'a mut VirtqAvailable,
    used: &'a mut VirtQUsed,
    irq: crate::gic::GIC,
}

#[repr(C)]
pub struct BlkReqHdr {
    pub req_type: LEU32,
    pub reserved: u32,
    pub sector: LEU64,
}

impl<'a> VirtIODevice<'a> for VirtIOBlk<'a> {
    unsafe fn new(
        regs: &'a mut VirtIORegs,
        desc: &'a mut [VirtQDesc],
        avail: &'a mut VirtqAvailable,
        used: &'a mut VirtQUsed,
        irq: crate::gic::GIC,
    ) -> Self {
        VirtIOBlk {
            regs,
            desc,
            avail,
            used,
            irq,
        }
    }
}

impl<'a> VirtIOBlk<'a> {
    pub fn read(&mut self, sector: u64, data: &mut [u8; 512]) {
        unsafe {
            let mut status: u8 = 0;
            let blkreq_hdr = BlkReqHdr {
                req_type: 0.into(),
                reserved: 0,
                sector: sector.into(),
            };

            write_volatile(&mut status, 0);

            write_volatile(
                &mut self.desc[0],
                VirtQDesc {
                    addr: (&blkreq_hdr as *const _ as u64).into(),
                    len: (core::mem::size_of::<BlkReqHdr>() as u32).into(),
                    flags: (1).into(),
                    next: 1.into(),
                },
            );

            write_volatile(
                &mut self.desc[1],
                VirtQDesc {
                    addr: (data.as_ptr() as *const _ as u64).into(),
                    len: (512).into(),
                    flags: (3).into(),
                    next: 2.into(),
                },
            );

            write_volatile(
                &mut self.desc[2],
                VirtQDesc {
                    addr: (&status as *const _ as u64).into(),
                    len: (1).into(),
                    flags: 2.into(),
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

    pub fn write(&mut self, sector: u64, data: &[u8; 512]) {
        unsafe {
            let mut status: u8 = 0;
            let blkreq_hdr = BlkReqHdr {
                req_type: 1.into(),
                reserved: 0,
                sector: sector.into(),
            };

            write_volatile(&mut status, 0);

            write_volatile(
                &mut self.desc[0],
                VirtQDesc {
                    addr: (&blkreq_hdr as *const _ as u64).into(),
                    len: (core::mem::size_of::<BlkReqHdr>() as u32).into(),
                    flags: (1).into(),
                    next: 1.into(),
                },
            );

            write_volatile(
                &mut self.desc[1],
                VirtQDesc {
                    addr: (data.as_ptr() as *const _ as u64).into(),
                    len: (512).into(),
                    flags: (1).into(),
                    next: 2.into(),
                },
            );

            write_volatile(
                &mut self.desc[2],
                VirtQDesc {
                    addr: (&status as *const _ as u64).into(),
                    len: (1).into(),
                    flags: 2.into(),
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
