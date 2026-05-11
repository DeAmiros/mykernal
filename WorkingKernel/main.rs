#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[derive(PartialEq)]
#[repr(u32)]
pub enum FdtToken {
    BeginNode = 0x0000_0001,
    EndNode = 0x0000_0002,
    Prop = 0x0000_0003,
    Nop = 0x0000_0004,
    End = 0x0000_0009,
}

fn parse_token(raw_token: u32) -> Option<FdtToken> {
    match raw_token {
        0x0000_0001 => Some(FdtToken::BeginNode),
        0x0000_0002 => Some(FdtToken::EndNode),
        0x0000_0003 => Some(FdtToken::Prop),
        0x0000_0004 => Some(FdtToken::Nop),
        0x0000_0009 => Some(FdtToken::End),
        _ => None,
    }
}

fn is_match(ptr: *const u8, len: usize, target: &[u8]) -> bool {
    if len != target.len() {
        return false;
    }
    let target_ptr = target.as_ptr();
    for i in 0..len {
        unsafe {
            if *ptr.add(i) != *target_ptr.add(i) {
                return false;
            }
        }
    }
    true
}

#[no_mangle]
pub extern "C" fn kernel_main(dtb_arg: *const u32) -> ! {
    // Default to 0x40000000 if no DTB pointer is passed (common in QEMU virt raw boot)
    let dtb_ptr = if dtb_arg.is_null() || (dtb_arg as u32) < 0x40000000 {
        0x4000_0000 as *const u32
    } else {
        dtb_arg
    };

    unsafe {
        if u32::from_be(*dtb_ptr) == 0xd00d_feed {
            let off_struct = u32::from_be(*dtb_ptr.add(2));
            let off_strings = u32::from_be(*dtb_ptr.add(3));

            let mut ptr = (dtb_ptr as *const u8).add(off_struct as usize) as *const u32;
            let strings_ptr = (dtb_ptr as *const u8).add(off_strings as usize);

            let mut current_node_is_uart = false;
            let mut current_node_reg: u32 = 0;

            loop {
                let token_raw = u32::from_be(*ptr);
                let token = match parse_token(token_raw) {
                    Some(t) => t,
                    None => break,
                };

                match token {
                    FdtToken::BeginNode => {
                        ptr = ptr.add(1);
                        let name_ptr = ptr as *const u8;
                        let mut name_len = 0;
                        while *name_ptr.add(name_len) != 0 {
                            name_len += 1;
                        }
                        let padded_name_len = (name_len + 1 + 3) & !3;
                        ptr = name_ptr.add(padded_name_len) as *const u32;
                        current_node_is_uart = false;
                        current_node_reg = 0;
                    }
                    FdtToken::EndNode => {
                        if current_node_is_uart && current_node_reg != 0 {
                            let uart = current_node_reg as *mut u32;
                            core::ptr::write_volatile(uart, b'H' as u32);
                            break;
                        }
                        ptr = ptr.add(1);
                    }
                    FdtToken::Prop => {
                        let len = u32::from_be(*ptr.add(1));
                        let name_off = u32::from_be(*ptr.add(2));
                        let name_ptr = strings_ptr.add(name_off as usize);
                        let data_ptr = ptr.add(3);

                        if is_match(name_ptr, 11, b"compatible\0") {
                            if is_match(data_ptr as *const u8, 10, b"arm,pl011\0") {
                                current_node_is_uart = true;
                            }
                        } else if is_match(name_ptr, 4, b"reg\0") {
                            if len >= 8 {
                                current_node_reg = u32::from_be(*data_ptr.add(1));
                            } else if len >= 4 {
                                current_node_reg = u32::from_be(*data_ptr);
                            }
                        }
                        let padded_len = (len as usize + 3) & !3;
                        ptr = ptr.add(3 + (padded_len / 4));
                    }
                    FdtToken::Nop => {
                        ptr = ptr.add(1);
                    }
                    FdtToken::End => break,
                }
            }
        }
    }

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        unsafe {
            let v1 = *s1.add(i);
            let v2 = *s2.add(i);
            if v1 != v2 {
                return v1 as i32 - v2 as i32;
            }
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn rust_begin_unwind(_info: &PanicInfo) -> ! {
    loop {}
}

#[export_name = "_RNvNtCs1M9dR2PfXm1_4core9panicking18panic_nounwind_fmt"]
pub extern "C" fn panic_nounwind_fmt() -> ! {
    loop {}
}
