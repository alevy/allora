use crate::utils::*;
use core::ptr::{read_volatile, write_volatile};

use super::{Status, VirtIORegs, VirtQDesc, VirtQUsed, VirtqAvailable};

type LEU16 = Endian<u16, Little>;

pub struct VirtIONet<'a> {
    regs: &'a mut VirtIORegs<VirtIONetConfig>,
    read_queue: &'a mut super::Queue<128>,
    write_queue: &'a mut super::Queue<128>,
    irq: crate::gic::GIC,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct VirtIONetConfig {
    pub mac: [u8; 6],
    pub status: LEU16,
    pub max_virtqueue_pairs: LEU16,
    pub mtu: LEU16,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct NetHdr {
    pub flags: u8,
    pub gso_type: u8,
    pub hdr_len: LEU16,
    pub gso_size: LEU16,
    pub csum_start: LEU16,
    pub csum_offset: LEU16,
}

const NET_DEVICE_FEATURES: u32 = 1 << 5; // VIRTIO_NET_F_MAC

impl<'a> VirtIONet<'a> {
    pub fn new(
        regs: &'a mut VirtIORegs<VirtIONetConfig>,
        read_queue: &'a mut super::Queue<128>,
        write_queue: &'a mut super::Queue<128>,
        irq: crate::gic::GIC,
    ) -> Self {
        unsafe {
            write_volatile(&mut regs.status, Status::Reset.into());
            write_volatile(&mut regs.status, Status::Acknowledge.into());
            write_volatile(&mut regs.status, Status::Driver.into());

            write_volatile(&mut regs.device_features_sel, 0.into());
            let device_features = read_volatile(&mut regs.device_features).native();
            write_volatile(&mut regs.driver_features_sel, 0.into());
            write_volatile(
                &mut regs.driver_features,
                (NET_DEVICE_FEATURES & device_features).into(),
            );

            write_volatile(&mut regs.status, Status::FeaturesOk.into());
            if read_volatile(&mut regs.status).native() & (Status::FeaturesOk as u32) == 0 {
                panic!("Coudln't set blk features");
            }

            for (i, queue) in [&read_queue, &write_queue].iter().enumerate() {
                write_volatile(&mut regs.queue_sel, (i as u32).into());
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
                    (&queue.available as *const VirtqAvailable as u32).into(),
                );
                write_volatile(
                    &mut regs.queue_avail_high,
                    ((&queue.available as *const VirtqAvailable as usize >> 32) as u32).into(),
                );
                write_volatile(
                    &mut regs.queue_used_low,
                    (&queue.used as *const VirtQUsed as u32).into(),
                );
                write_volatile(
                    &mut regs.queue_used_high,
                    ((&queue.used as *const VirtQUsed as usize >> 32) as u32).into(),
                );
                write_volatile(&mut regs.queue_ready, 1.into());
            }

            write_volatile(&mut regs.status, Status::DriverOk.into());
        }
        VirtIONet {
            regs,
            read_queue,
            write_queue,
            irq,
        }
    }
}

impl<'a> VirtIONet<'a> {
    pub fn config(&self) -> &VirtIONetConfig {
        unsafe { &*(&self.regs.config as *const _ as *const VirtIONetConfig) }
    }

    fn enqueue(&mut self, qnum: u32, descriptor: u16) {
        let queue = match qnum {
            0 => &mut self.read_queue,
            _ => &mut self.write_queue,
        };

        queue.available.ring[queue.available.idx.native() as usize % queue.available.ring.len()] =
            descriptor.into();
        mb();

        queue.available.idx = (queue.available.idx.native().wrapping_add(1)).into();
        mb();
        self.regs.queue_notify = qnum.into();
    }

    pub fn read(&mut self, data: &mut [u8; 1526]) {
        let mut blkreq_hdr = NetHdr {
            ..Default::default()
        };

        self.read_queue.descriptors[0] = VirtQDesc {
            addr: (&mut blkreq_hdr as *mut _ as u64).into(),
            len: (core::mem::size_of::<NetHdr>() as u32).into(),
            flags: (3).into(),
            next: 1.into(),
        };

        self.read_queue.descriptors[1] = VirtQDesc {
            addr: (data.as_ptr() as *const _ as u64).into(),
            len: (data.len() as u32).into(),
            flags: (2).into(),
            next: 0.into(),
        };

        self.enqueue(0, 0);

        unsafe {
            self.irq.enable();
            while read_volatile(&self.read_queue.used.idx)
                != read_volatile(&self.read_queue.available.idx)
            {
                asm!("wfi");
                let status = read_volatile(&self.regs.interrupt_status);
                if status.native() != 0 {
                    write_volatile(&mut self.regs.interrupt_ack, 0b11.into());
                }
            }
            self.irq.disable();
        }
    }

    pub fn write(&mut self, data: &[u8; 1526]) {
        let mut blkreq_hdr = NetHdr {
            ..Default::default()
        };

        self.write_queue.descriptors[0] = VirtQDesc {
            addr: (&mut blkreq_hdr as *mut _ as u64).into(),
            len: (core::mem::size_of::<NetHdr>() as u32).into(),
            flags: (1).into(),
            next: 1.into(),
        };

        self.write_queue.descriptors[1] = VirtQDesc {
            addr: (data.as_ptr() as *const _ as u64).into(),
            len: (data.len() as u32).into(),
            flags: (0).into(),
            next: 0.into(),
        };

        self.enqueue(1, 0);

        unsafe {
            self.irq.enable();
            while read_volatile(&self.write_queue.used.idx)
                != read_volatile(&self.write_queue.available.idx)
            {
                asm!("wfi");
                let status = read_volatile(&self.regs.interrupt_status);
                if status.native() != 0 {
                    write_volatile(&mut self.regs.interrupt_ack, 0b11.into());
                }
            }
            self.irq.disable();
        }
    }
}
