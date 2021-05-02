use core::ptr::{read_volatile, write_volatile};
use crate::utils::*;

#[derive(Debug)]
pub enum Status {
    Reset = 0,
    /// Indicates that the guest OS has found the device and recognized it as a valid virtio device.
    Acknowledge = 1,
    /// Indicates that the guest OS knows how to drive the device. Note: There could be a significant (or infinite) delay before setting this bit. For example, under Linux, drivers can be loadable modules.
    Driver = 2,
    /// Indicates that something went wrong in the guest, and it has given up on the device. This could be an internal error, or the driver didn’t like the device for some reason, or even a fatal error during device operation.
    Failed = 128,
    /// Indicates that the driver has acknowledged all the features it understands, and feature negotiation is complete.
    FeaturesOk = 8,
    /// Indicates that the driver is set up and ready to drive the device.
    DriverOk = 4,
    /// Indicates that the device has experienced an error from which it can’t recover.
    NeedsReset = 64,
}

impl Into<Endian<u32, Little>> for Status {
    fn into(self) -> Endian<u32, Little> {
        Endian::from(self as u32)
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum DeviceId {
    Invalid = 0,
    Net = 1,
    Blk = 2,
    Console = 3,
    Entropy = 4,
    MemoryBalloon = 5,
    IOMemory = 6,
    RPMSG = 7,
    SCSIHost = 8,
    NinePTransport = 9,
}

type LEU32 = Endian<u32, Little>;
type LEU64 = Endian<u64, Little>;

#[repr(C)]
pub struct VirtIORegs {
    pub magic: LEU32,
    pub version: LEU32,
    pub device_id: LEU32,
    pub vendor_id: LEU32,
    pub device_features: LEU32,
    pub device_features_sel: LEU32,
    _reserved0: [u32; 2],
    pub driver_features: LEU32,
    pub driver_features_sel: LEU32,
    _reserved1: [u32; 2],
    pub queue_sel: LEU32,
    pub queue_num_max: LEU32,
    pub queue_num: LEU32,
    _reserved2: [u32; 2],
    pub queue_ready: LEU32,
    _reserved3: [u32; 2],
    pub queue_notify: LEU32,
    _reserved4: [u32; 3],
    pub interrupt_status: LEU32,
    pub interrupt_ack: LEU32,
    _reserved5: [u32; 2],
    pub status: LEU32,
    _reserved6: [u32; 3],
    pub queue_desc_low: LEU32,
    pub queue_desc_high: LEU32,
    _reserved7: [u32; 2],
    pub queue_avail_low: LEU32,
    pub queue_avail_high: LEU32,
    _reserved8: [u32; 2],
    pub queue_used_low: LEU32,
    pub queue_used_high: LEU32,
    _reserved9: [u32; 21],
    pub config_generation: LEU32,
    pub config: LEU64,
}

const MAGIC: u32 = 0x74726976;

#[derive(Copy, Clone)]
#[repr(C,align(16))]
pub struct VirtQDesc {
    addr: LEU64,
    len: LEU32,
    flags: Endian<u16, Little>,
    next: Endian<u16, Little>,
}

impl VirtQDesc {
    pub const fn empty() -> VirtQDesc {
        VirtQDesc {
            addr: Endian::from_raw(0),
            len: Endian::from_raw(0),
            flags: Endian::from_raw(0),
            next: Endian::from_raw(0),
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C,align(2))]
pub struct VirtqAvailable { 
    flags: Endian<u16, Little>,
    idx: Endian<u16, Little>,
    ring: [Endian<u16, Little>; 128],
    used_event: Endian<u16, Little>,
}

impl VirtqAvailable {
    pub const fn empty() -> VirtqAvailable {
        VirtqAvailable {
            flags: Endian::from_raw(0),
            idx: Endian::from_raw(0),
            ring: [Endian::from_raw(0); 128],
            used_event: Endian::from_raw(0),
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C,packed)]
struct VirtQUsedElement {
    id: Endian<u16, Little>,
    len: Endian<u16, Little>,
}

impl VirtQUsedElement {
    pub const fn empty() -> VirtQUsedElement {
        VirtQUsedElement {
            id: Endian::from_raw(0),
            len: Endian::from_raw(0),
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C,align(4))]
pub struct VirtQUsed {
    flags: Endian<u16, Little>,
    idx: Endian<u16, Little>,
    ring: [VirtQUsedElement; 128],
    avail_event: Endian<u16, Little>,
}

impl VirtQUsed {
    pub const fn empty() -> VirtQUsed {
        VirtQUsed {
            flags: Endian::from_raw(0),
            idx: Endian::from_raw(0),
            ring: [VirtQUsedElement::empty(); 128],
            avail_event: Endian::from_raw(0),
        }
    }
}

impl VirtIORegs {
    pub unsafe fn new<'a>(base: *mut VirtIORegs) -> Option<&'a mut VirtIORegs> {
        let candidate = &mut *base;
        if candidate.magic.native() == MAGIC
            && candidate.version.native() == 2
            && candidate.device_id.native() != DeviceId::Invalid as u32
        {
            Some(candidate)
        } else {
            None
        }
    }

    pub fn device_id(&self) -> DeviceId {
        match self.device_id.native() {
            1 => DeviceId::Net,
            2 => DeviceId::Blk,
            3 => DeviceId::Console,
            4 => DeviceId::Entropy,
            5 => DeviceId::MemoryBalloon,
            6 => DeviceId::IOMemory,
            7 => DeviceId::RPMSG,
            8 => DeviceId::SCSIHost,
            9 => DeviceId::NinePTransport,
            _ => DeviceId::Invalid,
        }
    }
}

pub trait VirtIODevice<'a>: Sized {
    unsafe fn new(regs: &'a mut VirtIORegs, desc: &'a mut [VirtQDesc], avail: &'a mut VirtqAvailable, used: &'a mut VirtQUsed) -> Self;

    fn init(regs: &'a mut VirtIORegs, desc: &'a mut [VirtQDesc], avail: &'a mut VirtqAvailable, used: &'a mut VirtQUsed) -> Option<Self> {
        unsafe {
            write_volatile(&mut regs.status, Status::Reset.into());
            mb();
            write_volatile(&mut regs.status, Status::Acknowledge.into());
            mb();
            write_volatile(&mut regs.status, Status::Driver.into());
            mb();

            write_volatile(&mut regs.device_features_sel, 0.into());
            mb();
            let device_features = read_volatile(&mut regs.device_features).native();
            write_volatile(&mut regs.driver_features_sel, 0.into());
            mb();
            write_volatile(&mut regs.driver_features, (BLK_DEVICE_FEATURES & device_features).into());
            mb();

            write_volatile(&mut regs.status, Status::FeaturesOk.into());
            mb();
            if read_volatile(&mut regs.status).native() & (Status::FeaturesOk as u32) == 0 {
                panic!("Coudln't set blk features");
            }

            write_volatile(&mut regs.queue_sel, 0.into());
            mb();
            write_volatile(&mut regs.queue_num, (desc.len() as u32).into());
            write_volatile(&mut regs.queue_desc_low, ((desc.as_ptr() as usize) as u32).into());
            write_volatile(&mut regs.queue_desc_high, ((desc.as_ptr() as usize >> 32) as u32).into());
            write_volatile(&mut regs.queue_avail_low, (avail as *const _ as u32).into());
            write_volatile(&mut regs.queue_avail_high, ((avail as *const _ as usize >> 32) as u32).into());
            write_volatile(&mut regs.queue_used_low, (used as *const _ as u32).into());
            write_volatile(&mut regs.queue_used_high, ((used as *const _ as usize >> 32) as u32).into());
            mb();
            write_volatile(&mut regs.queue_ready, 1.into());
            mb();

            write_volatile(&mut regs.status, Status::DriverOk.into());
            mb();
            if read_volatile(&mut regs.status).native() & (Status::DriverOk as u32) == 0 {
                panic!("Coudln't set blk features");
            }
            Some(Self::new(
                regs,
                desc,
                avail,
                used,
            ))
        }
    }
}

const BLK_DEVICE_FEATURES: u32 = 0;

pub struct VirtIOBlk<'a> {
    regs: &'a mut VirtIORegs,
    desc: &'a mut [VirtQDesc],
    avail: &'a mut VirtqAvailable,
    used: &'a mut VirtQUsed,
}

#[repr(C)]
pub struct BlkReqHdr { 
    pub req_type: LEU32,
    pub reserved: u32,
    pub sector: LEU64,
}

impl<'a> VirtIODevice<'a> for VirtIOBlk<'a> {
    unsafe fn new(regs: &'a mut VirtIORegs, desc: &'a mut [VirtQDesc], avail: &'a mut VirtqAvailable, used: &'a mut VirtQUsed) -> Self {
        VirtIOBlk {
            regs,
            desc,
            avail,
            used,
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

            write_volatile(&mut self.desc[0], VirtQDesc {
                addr: (&blkreq_hdr as *const _ as u64).into(),
                len: (core::mem::size_of::<BlkReqHdr>() as u32).into(),
                flags: (1).into(),
                next: 1.into(),
            });

            write_volatile(&mut self.desc[1], VirtQDesc {
                addr: (data.as_ptr() as *const _ as u64).into(),
                len: (512).into(),
                flags: (3).into(),
                next: 2.into(),
            });

            write_volatile(&mut self.desc[2], VirtQDesc {
                addr: (&status as *const _ as u64).into(),
                len: (1).into(),
                flags: 2.into(),
                next: 0.into(),
            });

            write_volatile(&mut self.avail.ring[self.avail.idx.native() as usize], 0.into());
            mb();
            write_volatile(&mut self.avail.idx, (self.avail.idx.native() + 1).into());
            mb();
            write_volatile(&mut self.regs.queue_notify, 0.into());
            mb();
            while read_volatile(&self.used.idx).native() != read_volatile(&self.avail.idx).native() {}
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

            write_volatile(&mut self.desc[0], VirtQDesc {
                addr: (&blkreq_hdr as *const _ as u64).into(),
                len: (core::mem::size_of::<BlkReqHdr>() as u32).into(),
                flags: (1).into(),
                next: 1.into(),
            });

            write_volatile(&mut self.desc[1], VirtQDesc {
                addr: (data.as_ptr() as *const _ as u64).into(),
                len: (512).into(),
                flags: (1).into(),
                next: 2.into(),
            });

            write_volatile(&mut self.desc[2], VirtQDesc {
                addr: (&status as *const _ as u64).into(),
                len: (1).into(),
                flags: 2.into(),
                next: 0.into(),
            });

            write_volatile(&mut self.avail.ring[self.avail.idx.native() as usize], 0.into());
            mb();
            write_volatile(&mut self.avail.idx, (self.avail.idx.native() + 1).into());
            mb();
            write_volatile(&mut self.regs.queue_notify, 0.into());
            mb();
            while read_volatile(&self.used.idx).native() != read_volatile(&self.avail.idx).native() {}
        }
    }
}

pub struct VirtIOEntropy<'a> {
    regs: &'a mut VirtIORegs,
    desc: &'a mut [VirtQDesc],
    avail: &'a mut VirtqAvailable,
    used: &'a mut VirtQUsed,
}

impl<'a> VirtIODevice<'a> for VirtIOEntropy<'a> {
    unsafe fn new(regs: &'a mut VirtIORegs, desc: &'a mut [VirtQDesc], avail: &'a mut VirtqAvailable, used: &'a mut VirtQUsed) -> Self {
        VirtIOEntropy {
            regs,
            desc,
            avail,
            used,
        }
    }
}

impl<'a> VirtIOEntropy<'a> {
    pub fn read(&mut self, data: &mut [u8]) {
        unsafe {
            write_volatile(&mut self.desc[0], VirtQDesc {
                addr: (data.as_ptr() as *const _ as u64).into(),
                len: (data.len() as u32).into(),
                flags: (2).into(),
                next: 0.into(),
            });

            write_volatile(&mut self.avail.ring[self.avail.idx.native() as usize], 0.into());
            mb();
            write_volatile(&mut self.avail.idx, (self.avail.idx.native() + 1).into());
            mb();
            write_volatile(&mut self.regs.queue_notify, 0.into());
            mb();
            while read_volatile(&self.used.idx).native() != read_volatile(&self.avail.idx).native() { }
        }
    }
}
