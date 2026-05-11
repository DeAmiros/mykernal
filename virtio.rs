use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{fence, Ordering};

use crate::uart;
use crate::utils;

const QUEUE_SIZE: usize = 256;
const VIRTQ_DESC_F_WRITE: u16 = 2;

const VIRTIO_STATUS_ACKNOWLEDGE: u32 = 1;
const VIRTIO_STATUS_DRIVER: u32 = 2;
const VIRTIO_STATUS_DRIVER_OK: u32 = 4;
const VIRTIO_STATUS_FEATURES_OK: u32 = 8;
const VIRTIO_STATUS_FAILED: u32 = 128;

const VIRTIO_NET_F_MAC: u32 = 1 << 5;
const VIRTIO_NET_F_MRG_RXBUF: u32 = 1 << 15;
const VIRTIO_NET_F_STATUS: u32 = 1 << 16;
const VIRTIO_F_VERSION_1: u32 = 1;

#[repr(C, packed)]
pub struct VirtioMmio {
    pub magic: u32,               // 0x00
    pub version: u32,             // 0x04
    pub device_id: u32,           // 0x08
    pub vendor_id: u32,           // 0x0c
    pub device_features: u32,     // 0x10
    pub device_features_sel: u32, // 0x14
    reserved1: [u32; 2],          // 0x18 - 0x1c (Padding)
    pub driver_features: u32,     // 0x20
    pub driver_features_sel: u32, // 0x24
    reserved2: [u32; 2],          // 0x28 - 0x2c (Padding)
    pub queue_sel: u32,           // 0x30
    pub queue_num_max: u32,       // 0x34
    pub queue_num: u32,           // 0x38
    reserved3: [u32; 2],          // 0x3c - 0x40 (Padding)
    pub queue_ready: u32,         // 0x44
    reserved4: [u32; 2],          // 0x48 - 0x4c (Padding)
    pub queue_notify: u32,        // 0x50
    reserved5: [u32; 3],          // 0x54 - 0x5c (Padding)
    pub interrupt_status: u32,    // 0x60
    pub interrupt_ack: u32,       // 0x64
    reserved6: [u32; 2],          // 0x68 - 0x6c (Padding)
    pub status: u32,              // 0x70
    reserved7: [u32; 3],          // 0x74 - 0x7c (Padding)
    pub queue_desc_low: u32,      // 0x80
    pub queue_desc_high: u32,     // 0x84
    reserved8: [u32; 2],          // 0x88 - 0x8c (Padding)
    pub queue_avail_low: u32,     // 0x90
    pub queue_avail_high: u32,    // 0x94
    reserved9: [u32; 2],          // 0x98 - 0x9c (Padding)
    pub queue_used_low: u32,      // 0xa0
    pub queue_used_high: u32,     // 0xa4
    reserved10: [u32; 22],        // 0xa8 - 0xfc (Padding)],
    mac_addr: [u8; 6],            // 0x100 - 0x105
}

#[repr(C, align(4096))]
pub struct Virtqueue {
    pub descriptors: [VirtqDescriptor; 256],
    pub available: VirtqAvail,
    // Add padding to align the Used ring to 4096 bytes (8192 - 4614 = 3578)
    pub padding: [u8; 3578],
    pub used: VirtqUsed,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VirtqDescriptor {
    pub addr: u64,  // 64-bit Address of the packet data in RAM
    pub len: u32,   // 32-bit Length of the packet
    pub flags: u16, // 16-bit Flags (e.g., is there a "next" descriptor?)
    pub next: u16,  // 16-bit Index of the next descriptor in the chain
}

#[repr(C, packed)]
pub struct VirtqAvail {
    pub flags: u16,       // Options (Usually set to 0)
    pub idx: u16,         // The "Counter" (Increments every time you add an item)
    pub ring: [u16; 256], // The actual list of Descriptor indices
    pub used_event: u16,  // (Leave at 0)
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VirtqUsedElem {
    pub id: u32,  // The index of the Descriptor the hardware just finished
    pub len: u32, // The total number of bytes the hardware moved
}
#[repr(C, packed)]
pub struct VirtqUsed {
    pub flags: u16,
    pub idx: u16, // The hardware's counter (Increments when it finishes an item)
    pub ring: [VirtqUsedElem; 256],
    pub avail_event: u16, // (The hardware looks at this to see when to interrupt YOU)
}

pub static mut RX_QUEUE: Virtqueue = Virtqueue {
    descriptors: [VirtqDescriptor {
        addr: 0,
        len: 0,
        flags: 0,
        next: 0,
    }; 256],
    available: VirtqAvail {
        flags: 0,
        idx: 0,
        ring: [0; 256],
        used_event: 0,
    },
    padding: [0; 3578],
    used: VirtqUsed {
        flags: 0,
        idx: 0,
        ring: [VirtqUsedElem { id: 0, len: 0 }; 256],
        avail_event: 0,
    },
};

pub static mut TX_QUEUE: Virtqueue = Virtqueue {
    descriptors: [VirtqDescriptor {
        addr: 0,
        len: 0,
        flags: 0,
        next: 0,
    }; 256],
    available: VirtqAvail {
        flags: 0,
        idx: 0,
        ring: [0; 256],
        used_event: 0,
    },
    padding: [0; 3578],
    used: VirtqUsed {
        flags: 0,
        idx: 0,
        ring: [VirtqUsedElem { id: 0, len: 0 }; 256],
        avail_event: 0,
    },
};

pub fn plug_in_queues() {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_sel), 0); // Select RX queue
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_num), 256); // Set queue size
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_desc_low),
            core::ptr::addr_of!(RX_QUEUE.descriptors) as u32,
        ); // Set descriptor table address
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_desc_high), 0x0);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_avail_low),
            core::ptr::addr_of!(RX_QUEUE.available) as u32,
        ); // Set available ring address
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_avail_high),
            0x0,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_used_low),
            core::ptr::addr_of!(RX_QUEUE.used) as u32,
        ); // Set used ring address
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_used_high), 0x0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_ready), 1); // Mark the queue as ready
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_sel), 1); // Select TX queue
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_num), 256); // Set queue size
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_desc_low),
            core::ptr::addr_of!(TX_QUEUE.descriptors) as u32,
        ); // Set descriptor table address
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_desc_high), 0x0);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_avail_low),
            core::ptr::addr_of!(TX_QUEUE.available) as u32,
        ); // Set available ring address
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_avail_high),
            0x0,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_used_low),
            core::ptr::addr_of!(TX_QUEUE.used) as u32,
        ); // Set used ring address
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_used_high), 0x0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_ready), 1); // Mark the queue as ready

        for i in 0..QUEUE_SIZE {
            RX_QUEUE.descriptors[i].addr = &RX_BUFFERS[i] as *const _ as u64;
            RX_QUEUE.descriptors[i].len = core::mem::size_of::<RawPacket>() as u32;
            RX_QUEUE.descriptors[i].flags = VIRTQ_DESC_F_WRITE;
            RX_QUEUE.descriptors[i].next = 0; // No next descriptor
            RX_QUEUE.available.ring[i] = i as u16; // Add to available ring
        }
        fence(Ordering::SeqCst);
        core::ptr::write_volatile(core::ptr::addr_of_mut!(RX_QUEUE.available.idx), 256);
        // Update available index
    }
}

pub fn notify_rx_queue() {
    unsafe {
        fence(Ordering::SeqCst);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_notify), 0);
    }
}

static mut last_used_rx_idx: u16 = 0;
pub fn pull_rx() {
    unsafe {
        loop {
            let current_used_idx = core::ptr::read_volatile(core::ptr::addr_of!(RX_QUEUE.used.idx));
            if current_used_idx == last_used_rx_idx {
                break;
            }

            fence(Ordering::SeqCst);
            let ring_idx: usize = (last_used_rx_idx % QUEUE_SIZE as u16) as usize;
            let ring_ptr = RX_QUEUE.used.ring.as_ptr().add(ring_idx);
            let desc_idx =
                core::ptr::read_volatile(core::ptr::addr_of!((*ring_ptr).id)) as usize % QUEUE_SIZE;
            let len = core::ptr::read_volatile(core::ptr::addr_of!((*ring_ptr).len)) as usize;

            uart::write_str("\nPacket Received! Length: ");
            utils::print_hex((len >> 8) as u8);
            utils::print_hex(len as u8);
            uart::write_str("\n");

            let hdr_len: usize = core::mem::size_of::<VirtioNetHeader>();
            let payload_len = if len > hdr_len { len - hdr_len } else { 0 };

            let safe_desc_idx = desc_idx % 256;
            let max_data_len = 1514; // Size of RawPacket::data
            let actual_len = if payload_len > max_data_len {
                max_data_len
            } else {
                payload_len
            };

            let packet_id_from_ring =
                core::ptr::read_volatile(RX_QUEUE.used.ring.as_ptr().add(ring_idx) as *const u16)
                    as usize;

            let packet_addr_ptr = core::ptr::read_volatile(
                RX_QUEUE.descriptors.as_ptr().add(packet_id_from_ring) as *const u64,
            ) as *const u32;

            let eth_addr =
                (packet_addr_ptr as *const VirtioNetHeader).add(1) as *const EthernetHeader;

            let ether_data = eth_addr.read_volatile();
            let ether_type = u16::from_be(ether_data.ether_type); // Convert from big-endian to host byte order

            match identify_packet_type(ether_type) {
                PacketType::ARP => {
                    let arp_frame = unsafe { eth_addr as *const ArpFrame }.read_volatile();
                    uart::write_str("ARP Packet: ");
                    utils::print_hex(arp_frame.arp_payload.oper as u8);
                    uart::write_str("\n");
                }
                PacketType::IPv4 => {
                    uart::write_str("IPv4 Packet\n");
                }
                PacketType::IPv6 => {
                    uart::write_str("IPv6 Packet\n");
                }
                PacketType::Unknown => {
                    uart::write_str("Unknown Packet Type\n");
                }
            }

            last_used_rx_idx = last_used_rx_idx.wrapping_add(1);

            let avail_idx = core::ptr::read_volatile(core::ptr::addr_of!(RX_QUEUE.available.idx));
            let ring_avail_idx = (avail_idx % QUEUE_SIZE as u16) as usize;
            RX_QUEUE.available.ring[ring_avail_idx] = desc_idx as u16;
            fence(Ordering::SeqCst);
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!(RX_QUEUE.available.idx),
                avail_idx.wrapping_add(1),
            );
            fence(Ordering::SeqCst);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_notify), 0);
        }
    }
}

pub enum PacketType {
    ARP,
    IPv4,
    IPv6,
    Unknown,
}

pub fn identify_packet_type(ether_type: u16) -> PacketType {
    match ether_type {
        0x0806 => PacketType::ARP,
        0x0800 => PacketType::IPv4,
        0x86DD => PacketType::IPv6,
        _ => PacketType::Unknown,
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct VirtioNetHeader {
    pub flags: u8,
    pub gso_type: u8,
    pub hdr_len: u16,
    pub gso_size: u16,
    pub csum_start: u16,
    pub csum_offset: u16,
    pub num_buffers: u16,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct RawPacket {
    pub header: VirtioNetHeader,
    pub data: [u8; 1514], // 1514 is the standard maximum size of an Ethernet packet
}
// Reserve the space for one Transmit buffer
static mut TX_BUFFERS: [RawPacket; 256] = [RawPacket {
    header: VirtioNetHeader {
        flags: 0,
        gso_type: 0,
        hdr_len: 0,
        gso_size: 0,
        csum_start: 0,
        csum_offset: 0,
        num_buffers: 0,
    },
    data: [0; 1514],
}; 256];

static mut RX_BUFFERS: [RawPacket; 256] = [RawPacket {
    header: VirtioNetHeader {
        flags: 0,
        gso_type: 0,
        hdr_len: 0,
        gso_size: 0,
        csum_start: 0,
        csum_offset: 0,
        num_buffers: 0,
    },
    data: [0; 1514],
}; 256];

#[repr(C, packed)]
pub struct EthernetHeader {
    pub dest_mac: [u8; 6],
    pub src_mac: [u8; 6],
    pub ether_type: u16,
}

#[repr(C, packed)]
pub struct ARP {
    pub htype: u16,
    pub ptype: u16,
    pub hlen: u8,
    pub plen: u8,
    pub oper: u16,
    pub sha: [u8; 6],
    pub spa: [u8; 4],
    pub tha: [u8; 6],
    pub tpa: [u8; 4],
}

#[repr(C, packed)]
pub struct ArpFrame {
    pub header: VirtioNetHeader,
    pub eth_header: EthernetHeader,
    pub arp_payload: ARP,
}

pub fn send_arp_request(target_ip: [u8; 4]) {
    unsafe {
        let source_mac = (*VIRTIO_MMIO).mac_addr;

        let arp_frame = ArpFrame {
            header: VirtioNetHeader {
                flags: 0,
                gso_type: 0,
                hdr_len: 0,
                gso_size: 0,
                csum_start: 0,
                csum_offset: 0,
                num_buffers: 0,
            },
            eth_header: EthernetHeader {
                dest_mac: [0xff; 6],           // Broadcast
                src_mac: source_mac,           // Example source MAC
                ether_type: 0x0806u16.to_be(), // ARP
            },
            arp_payload: ARP {
                htype: 1u16.to_be(),      // Ethernet
                ptype: 0x0800u16.to_be(), // IPv4
                hlen: 6,
                plen: 4,
                oper: 1u16.to_be(),  // ARP Request
                sha: source_mac,     // Example source MAC
                spa: [10, 0, 2, 15], // Example source IP
                tha: [0; 6],         // Target MAC (unknown)
                tpa: target_ip,      // Target IP
            },
        };
        send_packet(&arp_frame);
    }
}

pub fn send_packet(data_arp_frame: &ArpFrame) {
    unsafe {
        let tx = core::ptr::addr_of_mut!(TX_BUFFERS[0]);

        // Copy the data into the transmit buffer
        let size_of_frame = core::mem::size_of::<ArpFrame>();
        let mut final_size = size_of_frame - core::mem::size_of::<VirtioNetHeader>(); // Exclude the VirtioNetHeader from the length
        let data_ptr = data_arp_frame as *const ArpFrame as *const u8;
        core::ptr::copy_nonoverlapping(data_ptr, tx as *mut u8, size_of_frame);

        if final_size < 60 {
            for i in final_size..60 {
                write_volatile((*tx).data.as_mut_ptr().add(i), 0);
            }
            final_size = 60;
        }

        TX_QUEUE.descriptors.as_mut_ptr().write(VirtqDescriptor {
            addr: tx as *const RawPacket as u64,
            len: size_of_frame as u32,
            flags: 0,
            next: 0,
        });

        // Add the descriptor index to the available ring
        let current_idx = core::ptr::read_volatile(core::ptr::addr_of!(TX_QUEUE.available.idx));
        let avail_idx: usize = current_idx as usize % QUEUE_SIZE;
        let ads = (core::ptr::addr_of_mut!(TX_QUEUE.available.ring) as *const u16).add(avail_idx)
            as *mut u16;
        core::ptr::write_volatile(ads, 0u16); // Descriptor index 0

        fence(Ordering::SeqCst);

        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(TX_QUEUE.available.idx),
            current_idx.wrapping_add(1),
        );

        fence(Ordering::SeqCst);

        // Notify the device that a new packet is available
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_notify), 1);
    }
}

static mut TX_REPORTED: bool = false;

pub fn get_tx_used() -> u16 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(TX_QUEUE.used.idx)) }
}

pub fn get_tx_reported() -> bool {
    unsafe { TX_REPORTED }
}

pub fn set_tx_reported() {
    unsafe {
        TX_REPORTED = true;
    }
}

static mut VIRTIO_MMIO: *mut VirtioMmio = 0x0 as *mut VirtioMmio;

pub fn virtio_net_found() -> bool {
    unsafe { VIRTIO_MMIO != 0x0 as *mut VirtioMmio }
}

pub fn init_virtio(magic: *const u32) {
    unsafe {
        let virtio_mmio = magic as *mut VirtioMmio;

        if core::ptr::read_volatile(core::ptr::addr_of!((*virtio_mmio).magic)) == 0x74726976 {
            if core::ptr::read_volatile(core::ptr::addr_of!((*virtio_mmio).device_id)) != 1 {
                uart::write_str("Device is not a network card!\n");
                return;
            }

            if core::ptr::read_volatile(core::ptr::addr_of!((*virtio_mmio).version)) == 2 {
                VIRTIO_MMIO = virtio_mmio;

                core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).status), 0);
                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*VIRTIO_MMIO).status),
                    VIRTIO_STATUS_ACKNOWLEDGE,
                );
                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*VIRTIO_MMIO).status),
                    VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER,
                );

                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*VIRTIO_MMIO).device_features_sel),
                    0,
                );
                let device_features_0 =
                    core::ptr::read_volatile(core::ptr::addr_of!((*VIRTIO_MMIO).device_features));
                let driver_features_0 = device_features_0
                    & (VIRTIO_NET_F_MAC | VIRTIO_NET_F_MRG_RXBUF | VIRTIO_NET_F_STATUS);

                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*VIRTIO_MMIO).device_features_sel),
                    1,
                );
                let device_features_1 =
                    core::ptr::read_volatile(core::ptr::addr_of!((*VIRTIO_MMIO).device_features));
                if (device_features_1 & VIRTIO_F_VERSION_1) == 0 {
                    uart::write_str("Device does not support VIRTIO_F_VERSION_1!\n");
                    core::ptr::write_volatile(
                        core::ptr::addr_of_mut!((*VIRTIO_MMIO).status),
                        VIRTIO_STATUS_FAILED,
                    );
                    return;
                }

                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*VIRTIO_MMIO).driver_features_sel),
                    0,
                );
                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*VIRTIO_MMIO).driver_features),
                    driver_features_0,
                );

                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*VIRTIO_MMIO).driver_features_sel),
                    1,
                );
                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*VIRTIO_MMIO).driver_features),
                    VIRTIO_F_VERSION_1,
                );

                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*VIRTIO_MMIO).status),
                    VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_FEATURES_OK,
                );

                let status = core::ptr::read_volatile(core::ptr::addr_of!((*VIRTIO_MMIO).status));
                if (status & VIRTIO_STATUS_FEATURES_OK) == 0 {
                    uart::write_str("FEATURES_OK rejected by device!\n");
                    core::ptr::write_volatile(
                        core::ptr::addr_of_mut!((*VIRTIO_MMIO).status),
                        status | VIRTIO_STATUS_FAILED,
                    );
                    return;
                }

                plug_in_queues();

                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*VIRTIO_MMIO).status),
                    VIRTIO_STATUS_ACKNOWLEDGE
                        | VIRTIO_STATUS_DRIVER
                        | VIRTIO_STATUS_FEATURES_OK
                        | VIRTIO_STATUS_DRIVER_OK,
                );
                notify_rx_queue();
            }
        }
    }
}

pub fn print_mac_addr() {
    unsafe {
        if VIRTIO_MMIO != 0x0 as *mut VirtioMmio {
            let mac_addr_base = &(*VIRTIO_MMIO).mac_addr;

            for i in 0..6 {
                let byte = core::ptr::read_volatile(core::ptr::addr_of!(mac_addr_base[i]));
                utils::print_hex(byte);
                uart::send_byte(if i < 5 { b':' } else { b'\n' });
            }
        }
    }
}
