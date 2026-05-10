use crate::uart;

#[no_mangle]
pub fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let byte1 = unsafe { *s1.add(i) };
        let byte2 = unsafe { *s2.add(i) };

        if byte1 != byte2 {
            return byte1 as i32 - byte2 as i32;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn __aeabi_memcpy(dest: *mut u8, src: *const u8, n: usize)  {
    for i in 0..n {
        unsafe {
            *dest.add(i) = *src.add(i);
        }
    }
}

#[no_mangle]
pub extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    __aeabi_memcpy(dest, src, n);
    dest
}


#[no_mangle]
pub fn strlen(s: *const u8) -> usize {
    let mut len = 0;
    while unsafe { *s.add(len) } != 0 {
        len += 1;
    }
    len
}

pub fn print_hex(value: u8) {
    let hex_chars = b"0123456789abcdef";
    for i in (0..2).rev() {
        let nibble = ((value >> (i * 4)) & 0xf) as usize;
        uart::send_byte(hex_chars[nibble]);
    }
}

#[no_mangle]
pub fn parse_token(raw_token: u32) -> Result<FdtToken, &'static str> {
    match raw_token {
        0x0000_0001 => Ok(FdtToken::BeginNode),
        0x0000_0002 => Ok(FdtToken::EndNode),
        0x0000_0003 => Ok(FdtToken::Prop),
        0x0000_0004 => Ok(FdtToken::Nop),
        0x0000_0009 => Ok(FdtToken::End),
        _ => Err("Invalid hardware token"),
    }
}

#[derive(PartialEq)]
#[repr(u32)]
pub enum FdtToken {
    BeginNode = 0x0000_0001,
    EndNode = 0x0000_0002,
    Prop = 0x0000_0003,
    Nop = 0x0000_0004,
    End = 0x0000_0009,
}


pub fn is_match(ptr: *const u8, len: usize, target: &[u8]) -> bool {
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
