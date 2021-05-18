use crate::utils::*;
use core::ptr::{read_volatile, write_volatile};

use super::{Queue, Status, VirtIORegs, VirtQDesc};

pub struct VirtIOEntropy<'a> {
    regs: &'a mut VirtIORegs,
    queue: &'a mut Queue<128>,
    irq: crate::gic::GIC,
}

impl<'a> VirtIOEntropy<'a> {
    pub fn new(regs: &'a mut VirtIORegs, queue: &'a mut Queue<128>, irq: crate::gic::GIC) -> Self {
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
            write_volatile(&mut regs.queue_num, (queue.descriptors.len() as u32).into());
            write_volatile(
                &mut regs.queue_desc_low,
                ((queue.descriptors.as_ptr() as usize) as u32).into(),
            );
            write_volatile(
                &mut regs.queue_desc_high,
                ((queue.descriptors.as_ptr() as usize >> 32) as u32).into(),
            );
            write_volatile(
                &mut regs.queue_avail_low,
                (&queue.available as *const _ as u32).into(),
            );
            write_volatile(
                &mut regs.queue_avail_high,
                ((&queue.available as *const _ as usize >> 32) as u32).into(),
            );
            write_volatile(
                &mut regs.queue_used_low,
                (&queue.used as *const _ as u32).into(),
            );
            write_volatile(
                &mut regs.queue_used_high,
                ((&queue.used as *const _ as usize >> 32) as u32).into(),
            );
            write_volatile(&mut regs.queue_ready, 1.into());

            write_volatile(&mut regs.status, Status::DriverOk.into());
        }
        VirtIOEntropy { regs, queue, irq }
    }
}

impl<'a> VirtIOEntropy<'a> {
    pub fn read(&mut self, data: &mut [u8]) {
        unsafe {
            write_volatile(
                &mut self.queue.descriptors[0],
                VirtQDesc {
                    addr: (data.as_ptr() as *const _ as u64).into(),
                    len: (data.len() as u32).into(),
                    flags: (2).into(),
                    next: 0.into(),
                },
            );

            write_volatile(
                &mut self.queue.available.ring[self.queue.available.idx.native() as usize],
                0.into(),
            );
            mb();
            write_volatile(
                &mut self.queue.available.idx,
                (self.queue.available.idx.native() + 1).into(),
            );
            mb();
            write_volatile(&mut self.regs.queue_notify, 0.into());
            mb();
            self.irq.enable();
            while read_volatile(&self.queue.used.idx).native()
                != read_volatile(&self.queue.available.idx).native()
            {
                //asm!("wfi");
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
