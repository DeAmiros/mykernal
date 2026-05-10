use core::ops::Add;

use crate::uart;
use crate::utils;

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
    // We add a bit of padding here to ensure the Used ring is on a clean boundary
    pub padding: [u8; 1024],
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

static mut RX_QUEUE: Virtqueue = Virtqueue {
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
    padding: [0; 1024],
    used: VirtqUsed {
        flags: 0,
        idx: 0,
        ring: [VirtqUsedElem { id: 0, len: 0 }; 256],
        avail_event: 0,
    },
};

static mut TX_QUEUE: Virtqueue = Virtqueue {
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
    padding: [0; 1024],
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
            &(RX_QUEUE.descriptors) as *const _ as u32,
        ); // Set descriptor table address
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_desc_high), 0x0);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_avail_low),
            &(RX_QUEUE.available) as *const _ as u32,
        ); // Set available ring address
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_avail_high),
            0x0,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_used_low),
            &(RX_QUEUE.used) as *const _ as u32,
        ); // Set used ring address
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_used_high), 0x0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_ready), 1); // Mark the queue as ready
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_sel), 1); // Select TX queue
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_num), 256); // Set queue size
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_desc_low),
            &(TX_QUEUE.descriptors) as *const _ as u32,
        ); // Set descriptor table address
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_desc_high), 0x0);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_avail_low),
            &(TX_QUEUE.available) as *const _ as u32,
        ); // Set available ring address
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_avail_high),
            0x0,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_used_low),
            &(TX_QUEUE.used) as *const _ as u32,
        ); // Set used ring address
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_used_high), 0x0);
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_ready), 1); // Mark the queue as ready

        for i in 0..RX_BUFFERS.len() {
            RX_QUEUE.descriptors[i].addr = &RX_BUFFERS[i] as *const _ as u64;
            RX_QUEUE.descriptors[i].len = core::mem::size_of::<RawPacket>() as u32;
            RX_QUEUE.descriptors[i].flags = 2; // No special flags
            RX_QUEUE.descriptors[i].next = 0; // No next descriptor
            RX_QUEUE.available.ring[i] = i as u16; // Add to available ring
        }
        core::ptr::write_volatile(core::ptr::addr_of_mut!(RX_QUEUE.available.idx), 256); // Update available index
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_notify), 0);
        // Notify the device about the new buffers\

    }
}

static mut last_used_rx_idx: u16 = 0;
pub fn pull_rx() {
    unsafe {
        if RX_QUEUE.used.idx != last_used_rx_idx {
            last_used_rx_idx = RX_QUEUE.used.idx;

            uart::write_str("Pakcet Received!");
        }
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
    pub eth_header: EthernetHeader,
    pub arp_payload: ARP,
}

pub fn send_arp_request(target_ip: [u8; 4]) {
    unsafe {
        let source_mac = (*VIRTIO_MMIO).mac_addr;

        let arp_frame = ArpFrame {
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
                oper: 1u16.to_be(),      // ARP Request
                sha: source_mac,         // Example source MAC
                spa: [10, 0, 2, 15], // Example source IP
                tha: [0; 6],             // Target MAC (unknown)
                tpa: target_ip,          // Target IP
            },
        };
        send_packet(&arp_frame);
    }
}

pub fn send_packet(data_arp_fame: &ArpFrame) {
    unsafe {
        // Copy the data into the transmit buffer
        let size_of_frame = core::mem::size_of::<ArpFrame>();
        let data =
            core::slice::from_raw_parts(data_arp_fame as *const _ as *const u8, size_of_frame);
        TX_BUFFERS[0].data[..size_of_frame].copy_from_slice(&data[..size_of_frame]);

        // Set up the descriptor for the transmit buffer
        TX_QUEUE.descriptors[0].addr = &TX_BUFFERS[0] as *const _ as u64;
        TX_QUEUE.descriptors[0].len =
            (core::mem::size_of::<VirtioNetHeader>() + size_of_frame) as u32;
        TX_QUEUE.descriptors[0].flags = 0; // No special flags
        TX_QUEUE.descriptors[0].next = 0; // No next descriptor

        // Add the descriptor index to the available ring
        let avail_idx: usize = TX_QUEUE.available.idx as usize % 256;
        TX_QUEUE.available.ring[avail_idx] = 0; // Descriptor index 0
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(TX_QUEUE.available.idx),
            (avail_idx + 1) as u16,
        );

        // Notify the device that a new packet is available
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).queue_notify), 1);
    }
}

static mut VIRTIO_MMIO: *mut VirtioMmio = 0x0 as *mut VirtioMmio;

pub fn virtio_net_found() -> bool {
    unsafe { VIRTIO_MMIO != 0x0 as *mut VirtioMmio }
}

pub fn init_virtio(magic: *const u32) {
    unsafe {
        VIRTIO_MMIO = magic as *mut VirtioMmio;

        if core::ptr::read_volatile(core::ptr::addr_of!((*VIRTIO_MMIO).magic)) == 0x74726976 {
            if core::ptr::read_volatile(core::ptr::addr_of!((*VIRTIO_MMIO).version)) == 2 {
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).status), 0);
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).status), 1);
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*VIRTIO_MMIO).status), 2 | 1);
                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*VIRTIO_MMIO).status),
                    1 | 2 | 8,
                );
                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*VIRTIO_MMIO).status),
                    1 | 2 | 8 | 4,
                );

                plug_in_queues();
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
