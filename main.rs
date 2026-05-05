#![no_std]
#![no_main]

mod uart;
mod utils;

use core::{panic::PanicInfo, ptr::write_volatile};
use utils::*;
use uart::*;



#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    let dtb_ptr: *const u32 = 0x40_000_000 as *const u32;

    unsafe {
        // Convert from Big-Endian to the CPU's native order
        let magic = u32::from_be(*dtb_ptr);

        if magic == 0xd00d_feed {
            // Now we know 'magic' is correct, so we read the size

            set_special_addresses_from_dtb(dtb_ptr);

            loop {
                let c = get_c();

                send_byte(c.to_ascii_lowercase());
            }
        }

        loop {}
    }
}

fn set_special_addresses_from_dtb(dtb_ptr: *const u32, ) -> () {
    unsafe {
        let total_size = u32::from_be(*dtb_ptr.add(1));
        let structure_block_offset = u32::from_be(*dtb_ptr.add(2));
        let strings_block_offset = u32::from_be(*dtb_ptr.add(3));
        let mut list_of_hardware_pointer = dtb_ptr.add((structure_block_offset as usize) >>2 );

        let mut uart_address: u32 = 0;
        let mut found_uart = false;
        let mut current_node_reg_address: u32 = 0;

        loop {
            let list_of_hardware_value = u32::from_be(*list_of_hardware_pointer);

            match parse_token(list_of_hardware_value) {
                Ok(FdtToken::End) => {
                    // Handle the end of the structure block
                    break;
                }

                Ok(FdtToken::EndNode) => {
                    if found_uart && current_node_reg_address != 0 {
                        uart::init(current_node_reg_address);
                    }

                    list_of_hardware_pointer = list_of_hardware_pointer.add(1);
                }

                Ok(FdtToken::Prop) => {
                    let property_length = u32::from_be(*list_of_hardware_pointer.add(1));
                    let name_offset = u32::from_be(*list_of_hardware_pointer.add(2));

                    let byte_ptr: *const u8 = dtb_ptr as *const u8;

                    let name_ptr = byte_ptr
                        .add(strings_block_offset as usize)
                        .add(name_offset as usize);

                    if is_match(name_ptr, 4, b"reg\0") {
                        if property_length == 8 {
                            current_node_reg_address =
                                u32::from_be(*list_of_hardware_pointer.add(3));
                        } else {
                            current_node_reg_address =
                                u32::from_be(*list_of_hardware_pointer.add(4));
                        }
                    } else if is_match(name_ptr, 11, b"compatible\0") {
                        let driver_id_pointer = list_of_hardware_pointer.add(3) as *const u8;

                        if is_match(driver_id_pointer, 10, b"arm,pl011\0") {
                            found_uart = true;
                        }
                    }

                    // After setting everything up
                    list_of_hardware_pointer =
                        list_of_hardware_pointer.add(((property_length as usize + 3) >> 2) + 3);
                }

                Ok(FdtToken::BeginNode) => {
                    found_uart = false;
                    current_node_reg_address = 0;

                    let name_length = strlen(list_of_hardware_pointer.add(1) as *const u8);

                    // Move the pointer to the next token after the node name
                    list_of_hardware_pointer =
                        list_of_hardware_pointer.add(((name_length) >> 2) + 2);
                    // same as doing name_len + 1 + 3 ...
                }

                Ok(_) => {
                    list_of_hardware_pointer = list_of_hardware_pointer.add(1);

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
