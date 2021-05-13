use core::fmt::Write;
use core::str::from_utf8;

use crate::uart::UART;
use crate::utils::*;
use crate::virtio::VirtIONet;

type BEU16 = Endian<u16, Big>;

fn checksum(payload: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    for i in 0..(payload.len() / 2) {
        sum += (payload[i * 2 + 1] as u32) | ((payload[i * 2] as u32) << 8);
    }
    if payload.len() % 2 != 0 {
        sum += payload[payload.len() - 1] as u32;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    return !(sum as u16);
}

#[repr(C)]
#[derive(Debug, Default)]
struct EthernetHeader {
    dst_mac: [u8; 6],
    src_mac: [u8; 6],
    ethertype: BEU16,
}

#[repr(C)]
#[derive(Debug, Default)]
struct Arp {
    hw_type: BEU16,
    protocol_type: BEU16,
    hw_addr_len: u8,
    proto_addr_len: u8,
    operation: BEU16,
    sender_hw_addr: [u8; 6],
    sender_proto_addr: [u8; 4],
    target_hw_addr: [u8; 6],
    target_proto_addr: [u8; 4],
}

#[repr(C)]
#[derive(Debug, Default)]
struct IpHeader {
    version_ihl: u8,
    _iptype: u8,
    length: BEU16,
    id: BEU16,
    flags_offset: BEU16,
    ttl: u8,
    protocol: u8,
    checksum: BEU16,
    src_addr: [u8; 4],
    dst_addr: [u8; 4],
}

#[repr(C)]
#[derive(Debug, Default)]
struct ICMP {
    icmp_type: u8,
    code: u8,
    checksum: BEU16,
    id: BEU16,
    sequence_number: BEU16,
    data: [u8; 0],
}

pub struct Net<'a, 'b: 'a> {
    pub uart: &'a mut UART,
    pub net: &'a mut VirtIONet<'b>,
}

impl<'a, 'b> Net<'a, 'b> {
    pub fn serve_ping(&mut self, _words: &mut dyn Iterator<Item = &[u8]>) {
        let mut buf = [0; 1526];
        loop {
            self.net.read(&mut buf);
            let eth_header = unsafe { &mut *(buf.as_ptr() as *mut EthernetHeader) };
            let eth_payload = &buf[14..];
            match eth_header.ethertype.native() {
                0x0806 => {
                    let arp = unsafe { &mut *(eth_payload.as_ptr() as *mut Arp) };
                    if (
                        arp.hw_type.native(),
                        arp.protocol_type.native(),
                        arp.operation.native(),
                    ) == (1, 0x0800, 1)
                    {
                        // ARP request
                        if arp.target_proto_addr == [192, 168, 14, 4] {
                            eth_header.dst_mac = eth_header.src_mac;
                            eth_header.src_mac = self.net.config.mac;

                            arp.operation = 2.into();

                            arp.target_hw_addr = arp.sender_hw_addr;
                            arp.target_proto_addr = arp.sender_proto_addr;

                            arp.sender_hw_addr = self.net.config.mac;
                            arp.sender_proto_addr = [192, 168, 14, 4];
                            self.net.write(&mut buf);
                        }
                    }
                }
                0x0800 => {
                    let ip = unsafe { &mut *(eth_payload.as_ptr() as *mut IpHeader) };
                    let ip_payload = &eth_payload[20..];
                    if ip.dst_addr == [192, 168, 14, 4] {
                        if ip.protocol == 0x1 {
                            // ICMP
                            eth_header.dst_mac = eth_header.src_mac;
                            eth_header.src_mac = self.net.config.mac;

                            ip.dst_addr = ip.src_addr;
                            ip.src_addr = [192, 168, 14, 4];
                            ip.id = 0.into();
                            ip.flags_offset = 0.into();
                            ip.checksum = 0.into();
                            ip.checksum = checksum(&eth_payload[..20]).into();

                            let icmp = unsafe { &mut *(ip_payload.as_ptr() as *mut ICMP) };
                            icmp.icmp_type = 0.into();
                            icmp.checksum = 0.into();
                            icmp.checksum =
                                checksum(&ip_payload[..(ip.length.native() as usize - 20)]).into();

                            self.net.write(&mut buf);
                        } else if ip.protocol == 0x11 && ip_payload[2] == 0 && ip_payload[3] == 44 {
                            // UDP port 44
                            let length = (ip_payload[4] as usize) << 8 | ip_payload[5] as usize;
                            from_utf8(&ip_payload[8..length])
                                .ok()
                                .and_then(|pay| write!(&mut self.uart, "{}", pay).ok());
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
