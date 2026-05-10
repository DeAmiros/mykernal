use crate::utils::*;

static mut UART_ADDR: u32 = 0;

// This replaces your "Uart::new"
pub fn init_uart(base_address: u32) {
    unsafe {
        UART_ADDR = base_address;
    }
}

// These are now top-level functions, not methods
pub fn send_byte(byte: u8) {
    unsafe {
        if UART_ADDR != 0 {
            core::ptr::write_volatile(UART_ADDR as *mut u8, byte);
        }
    }
}

pub fn write_str(s: &str) {
    for byte in s.as_bytes() {
        send_byte(*byte);
    }
}

pub fn uart_getc() -> u8 {
    unsafe {
        if UART_ADDR != 0 {
            let flag_reg_ptr = (UART_ADDR + 0x18) as *const u8; // Assuming flag register is at offset 5
            while (core::ptr::read_volatile(flag_reg_ptr) & 0x10 ) != 0 {
                
            }
            return core::ptr::read_volatile(UART_ADDR as *const u8);
        }
    }
    0
}

