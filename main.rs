#![no_std]
#![no_main]

#[derive(PartialEq)]
#[repr(u32)]
pub enum FdtToken {
    BeginNode = 0x0000_0001,
    EndNode = 0x0000_0002,
    Prop = 0x0000_0003,
    Nop = 0x0000_0004,
    End = 0x0000_0009,
}

fn parse_token(raw_token: u32) -> Result<FdtToken, &'static str> {
    match raw_token {
        0x0000_0001 => Ok(FdtToken::BeginNode),
        0x0000_0002 => Ok(FdtToken::EndNode),
        0x0000_0003 => Ok(FdtToken::Prop),
        0x0000_0004 => Ok(FdtToken::Nop),
        0x0000_0009 => Ok(FdtToken::End),
        _ => Err("Invalid hardware token"),
    }
}

fn is_match(ptr: *const u8, len: usize, target: &[u8]) -> bool {
    let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
    slice == target
}

#[no_mangle]
pub extern "C" fn kernel_main(dtb_ptr: *const u32) -> ! {
    unsafe {
        // Read the raw value from memory
        let raw_magic = *dtb_ptr;

        // Convert from Big-Endian to the CPU's native order
        let magic = u32::from_be(raw_magic);

        if magic == 0xd00d_feed {
            // Now we know 'magic' is correct, so we read the size
            // and convert it the same way!
            let total_size = u32::from_be(*dtb_ptr.add(1));

            let structure_block_offset = u32::from_be(*dtb_ptr.add(2));

            let strings_block_offset = u32::from_be(*dtb_ptr.add(3));

            let mut list_of_hardware_pointer = dtb_ptr.add(structure_block_offset as usize / 4);
            let mut found_uart = false;

            loop {
                let list_of_hardware_value = u32::from_be(*list_of_hardware_pointer);

                match parse_token(list_of_hardware_value) {
                    Ok(FdtToken::End) => {
                        // Handle the end of the structure block
                        break;
                    }

                    Ok(FdtToken::Prop) => {
                        let property_length = u32::from_be(*list_of_hardware_pointer.add(1));
                        let name_offset = u32::from_be(*list_of_hardware_pointer.add(2));

                        let byte_ptr: *const u8 = dtb_ptr as *const u8;

                        let name_ptr = byte_ptr
                            .add(strings_block_offset as usize)
                            .add(name_offset as usize);

                        if found_uart {
                            if is_match(name_ptr, 4, b"reg\0") {
                                let reg: u32 = u32::from_be(*list_of_hardware_pointer.add(3));

                                // We already found a compatible property for the ns16550a UART driver, so we can skip this one
                                list_of_hardware_pointer = list_of_hardware_pointer
                                    .add((property_length as usize + 3) / 4 + 3);
                                found_uart = false; // Reset the flag for the next compatible property
                                continue;
                            }
                        }
                        else if is_match(name_ptr, 11, b"compatible\0") {
                            let UART_pointer = list_of_hardware_pointer;

                            let driver_id_pointer = list_of_hardware_pointer.add(3) as *const u8;

                            if is_match(driver_id_pointer, property_length as usize, b"ns16550a\0")
                            {
                                found_uart = true;
                                // We found the compatible property for the ns16550a UART driver
                                // You can now use this information to initialize the driver or perform other actions as needed
                            }
                        }

                        // After setting everything up
                        list_of_hardware_pointer =
                            list_of_hardware_pointer.add((property_length as usize + 3) / 4 + 3);
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

        loop {}
    }
}
