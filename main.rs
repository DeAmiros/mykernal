#![no_std]
#![no_main]

mod uart;
mod utils;
mod virtio;

use core::{panic::PanicInfo, ptr::write_volatile};
use uart::*;
use utils::*;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    let fdt_ptr: *const u32 = 0x40_000_000 as *const u32;

    unsafe {
        // Convert from Big-Endian to the CPU's native order
        let magic = u32::from_be(*fdt_ptr);

        if magic == 0xd00d_feed {
            // Now we know 'magic' is correct, so we read the size

            set_special_addresses_from_dtb(fdt_ptr);

            if virtio::virtio_net_found() {
                write_str("Virtio network device found!\n");
            } else {
                write_str("No Virtio network device found.\n");
            }

            virtio::print_mac_addr();

            virtio::send_arp_request([10, 0, 2, 2]);

            loop {
                virtio::pull_rx();
  
            }
        }

        loop {}
    }
}

fn set_special_addresses_from_dtb(fdt_ptr: *const u32) -> () {
    unsafe {
        let total_size = u32::from_be(*fdt_ptr.add(1));
        let structure_block_offset = u32::from_be(*fdt_ptr.add(2));
        let strings_block_offset = u32::from_be(*fdt_ptr.add(3));
        let mut node_ptr = fdt_ptr.add((structure_block_offset as usize) >> 2);

        let mut found_uart = false;
        let mut found_virtio = false;
        let mut base_addr: u32 = 0;

        loop {
            let tag = u32::from_be(*node_ptr);

            match parse_token(tag) {
                Ok(FdtToken::End) => {
                    // Handle the end of the structure block
                    break;
                }

                Ok(FdtToken::EndNode) => {
                    if base_addr != 0 {
                        if found_uart {
                            uart::init_uart(base_addr);
                        } else if found_virtio {
                            // virtio::init(base_addr);
                            virtio::init_virtio(base_addr as *const u32);
                        }
                    }

                    node_ptr = node_ptr.add(1);
                }

                Ok(FdtToken::Prop) => {
                    let property_length = u32::from_be(*node_ptr.add(1));
                    let name_offset = u32::from_be(*node_ptr.add(2));

                    let byte_ptr: *const u8 = fdt_ptr as *const u8;

                    let name_ptr = byte_ptr
                        .add(strings_block_offset as usize)
                        .add(name_offset as usize);

                    if is_match(name_ptr, 4, b"reg\0") {
                        if property_length == 8 {
                            base_addr = u32::from_be(*node_ptr.add(3));
                        } else {
                            base_addr = u32::from_be(*node_ptr.add(4));
                        }
                    } else if is_match(name_ptr, 11, b"compatible\0") {
                        let driver_id_pointer = node_ptr.add(3) as *const u8;

                        if is_match(driver_id_pointer, 10, b"arm,pl011\0") {
                            found_uart = true;
                        } else if is_match(driver_id_pointer, 12, b"virtio,mmio\0") {
                            found_virtio = true;
                        }
                    }

                    // After setting everything up
                    node_ptr = node_ptr.add(((property_length as usize + 3) >> 2) + 3);
                }

                Ok(FdtToken::BeginNode) => {
                    found_uart = false;
                    found_virtio = false;
                    base_addr = 0;

                    let name_length = strlen(node_ptr.add(1) as *const u8);

                    // Move the pointer to the next token after the node name
                    node_ptr = node_ptr.add(((name_length) >> 2) + 2);
                    // same as doing name_len + 1 + 3 ...
                }

                Ok(_) => {
                    node_ptr = node_ptr.add(1);

                    // Handle the token as needed
                    // For example, you could print it or store it in a data structure
                }
                Err(e) => {
                    // Handle the error, e.g., log it or ignore it
                    loop {}
                }
            }
        }
    }
}
