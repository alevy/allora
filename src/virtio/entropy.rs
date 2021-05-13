use crate::utils::*;
use core::ptr::{read_volatile, write_volatile};

use super::{Status, VirtIORegs, VirtQDesc, VirtQUsed, VirtqAvailable};

pub struct VirtIOEntropy<'a> {
    regs: &'a mut VirtIORegs,
    desc: &'a mut [VirtQDesc],
    avail: &'a mut VirtqAvailable,
    used: &'a mut VirtQUsed,
    irq: crate::gic::GIC,
}

impl<'a> VirtIOEntropy<'a> {
    pub fn new(
        regs: &'a mut VirtIORegs,
        desc: &'a mut [VirtQDesc],
        avail: &'a mut VirtqAvailable,
        used: &'a mut VirtQUsed,
        irq: crate::gic::GIC,
    ) -> Self {
        unsafe {
            write_volatile(&mut regs.status, Status::Reset.into());
            write_volatile(&mut regs.status, Status::Acknowledge.into());
            write_volatile(&mut regs.status, Status::Driver.into());

            write_volatile(&mut regs.device_features_sel, 0.into());
            let device_features = read_volatile(&mut regs.device_features).native();
            write_volatile(&mut regs.driver_features_sel, 0.into());
            write_volatile(&mut regs.driver_features, (0 & device_features).into());

            write_volatile(&mut regs.status, Status::FeaturesOk.into());
            if read_volatile(&mut regs.status).native() & (Status::FeaturesOk as u32) == 0 {
                panic!("Coudln't set entropy features");
            }

            write_volatile(&mut regs.queue_sel, 0.into());
            write_volatile(&mut regs.queue_num, (desc.len() as u32).into());
            write_volatile(
                &mut regs.queue_desc_low,
                ((desc.as_ptr() as usize) as u32).into(),
            );
            write_volatile(
                &mut regs.queue_desc_high,
                ((desc.as_ptr() as usize >> 32) as u32).into(),
            );
            write_volatile(&mut regs.queue_avail_low, (avail as *const _ as u32).into());
            write_volatile(
                &mut regs.queue_avail_high,
                ((avail as *const _ as usize >> 32) as u32).into(),
            );
            write_volatile(&mut regs.queue_used_low, (used as *const _ as u32).into());
            write_volatile(
                &mut regs.queue_used_high,
                ((used as *const _ as usize >> 32) as u32).into(),
            );
            write_volatile(&mut regs.queue_ready, 1.into());

            write_volatile(&mut regs.status, Status::DriverOk.into());
        }
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
                let status = read_volatile(&self.regs.interrupt_status);
                if status.native() != 0 {
                    write_volatile(&mut self.regs.interrupt_ack, status);
                    if read_volatile(&self.regs.interrupt_status).native() == status.native() {
                        panic!("{:#x}", self.regs.interrupt_status.native());
                    }
                }
            }
            self.irq.disable();
        }
    }
}
