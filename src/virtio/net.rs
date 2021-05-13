use crate::utils::*;
use core::ptr::{read_volatile, write_volatile};

use super::{Status, VirtIORegs, VirtQDesc, VirtQUsed, VirtqAvailable};

type LEU16 = Endian<u16, Little>;

pub struct VirtIONet<'a> {
    regs: &'a mut VirtIORegs,
    pub config: &'a VirtIONetConfig,
    desc: &'a mut [VirtQDesc],
    avail: &'a mut VirtqAvailable,
    used: &'a mut VirtQUsed,

    wdesc: &'a mut [VirtQDesc],
    wavail: &'a mut VirtqAvailable,
    wused: &'a mut VirtQUsed,
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
        regs: &'a mut VirtIORegs,
        desc: &'a mut [VirtQDesc],
        avail: &'a mut VirtqAvailable,
        used: &'a mut VirtQUsed,

        wdesc: &'a mut [VirtQDesc],
        wavail: &'a mut VirtqAvailable,
        wused: &'a mut VirtQUsed,
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

            write_volatile(&mut regs.queue_sel, 1.into());
            write_volatile(&mut regs.queue_num, (wdesc.len() as u32).into());
            write_volatile(
                &mut regs.queue_desc_low,
                ((wdesc.as_ptr() as usize) as u32).into(),
            );
            write_volatile(
                &mut regs.queue_desc_high,
                ((wdesc.as_ptr() as usize >> 32) as u32).into(),
            );
            write_volatile(
                &mut regs.queue_avail_low,
                (wavail as *const _ as u32).into(),
            );
            write_volatile(
                &mut regs.queue_avail_high,
                ((wavail as *const _ as usize >> 32) as u32).into(),
            );
            write_volatile(&mut regs.queue_used_low, (wused as *const _ as u32).into());
            write_volatile(
                &mut regs.queue_used_high,
                ((wused as *const _ as usize >> 32) as u32).into(),
            );
            write_volatile(&mut regs.queue_ready, 1.into());

            write_volatile(&mut regs.status, Status::DriverOk.into());
        }
        let config = unsafe { &*(&regs.config as *const _ as *const VirtIONetConfig) };
        VirtIONet {
            regs,
            config,
            desc,
            avail,
            used,
            wdesc,
            wavail,
            wused,
            irq,
        }
    }
}

impl<'a> VirtIONet<'a> {
    pub fn read(&mut self, data: &mut [u8; 1526]) -> NetHdr {
        unsafe {
            let mut blkreq_hdr = NetHdr {
                ..Default::default()
            };

            write_volatile(
                &mut self.desc[0],
                VirtQDesc {
                    addr: (&mut blkreq_hdr as *mut _ as u64).into(),
                    len: (core::mem::size_of::<NetHdr>() as u32).into(),
                    flags: (3).into(),
                    next: 1.into(),
                },
            );

            write_volatile(
                &mut self.desc[1],
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
            write_volatile(&mut self.avail.idx, (self.avail.idx.native() + 1).into());
            write_volatile(&mut self.regs.queue_notify, 0.into());
            self.irq.enable();
            while read_volatile(&self.used.idx).native() != read_volatile(&self.avail.idx).native()
            {
                asm!("wfi");
                let status = read_volatile(&self.regs.interrupt_status);
                if status.native() != 0 {
                    write_volatile(&mut self.regs.interrupt_ack, status);
                }
            }
            self.irq.disable();
            read_volatile(self.desc[0].addr.native() as *const NetHdr)
        }
    }

    pub fn write(&mut self, data: &mut [u8; 1526]) -> NetHdr {
        unsafe {
            let mut blkreq_hdr = NetHdr {
                ..Default::default()
            };

            write_volatile(
                &mut self.wdesc[0],
                VirtQDesc {
                    addr: (&mut blkreq_hdr as *mut _ as u64).into(),
                    len: (core::mem::size_of::<NetHdr>() as u32).into(),
                    flags: (1).into(),
                    next: 1.into(),
                },
            );

            write_volatile(
                &mut self.wdesc[1],
                VirtQDesc {
                    addr: (data.as_ptr() as *const _ as u64).into(),
                    len: (data.len() as u32).into(),
                    flags: (0).into(),
                    next: 0.into(),
                },
            );

            write_volatile(
                &mut self.wavail.ring[self.wavail.idx.native() as usize],
                0.into(),
            );
            write_volatile(&mut self.wavail.idx, (self.wavail.idx.native() + 1).into());
            write_volatile(&mut self.regs.queue_notify, 1.into());
            self.irq.enable();
            while read_volatile(&self.wused.idx).native()
                != read_volatile(&self.wavail.idx).native()
            {
                asm!("wfi");
                let status = read_volatile(&self.regs.interrupt_status);
                if status.native() != 0 {
                    write_volatile(&mut self.regs.interrupt_ack, status);
                }
            }
            self.irq.disable();
            read_volatile(self.wdesc[0].addr.native() as *const NetHdr)
        }
    }
}
