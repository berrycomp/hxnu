use core::arch::asm;
use core::fmt::Write;

const COM1_PORT: u16 = 0x3F8;
const DATA_REGISTER: u16 = COM1_PORT;
const INTERRUPT_ENABLE_REGISTER: u16 = COM1_PORT + 1;
const FIFO_CONTROL_REGISTER: u16 = COM1_PORT + 2;
const LINE_CONTROL_REGISTER: u16 = COM1_PORT + 3;
const MODEM_CONTROL_REGISTER: u16 = COM1_PORT + 4;
const LINE_STATUS_REGISTER: u16 = COM1_PORT + 5;
const TRANSMIT_HOLDING_REGISTER_EMPTY: u8 = 1 << 5;

pub fn init() {
    unsafe {
        outb(INTERRUPT_ENABLE_REGISTER, 0x00);
        outb(LINE_CONTROL_REGISTER, 0x80);
        outb(DATA_REGISTER, 0x03);
        outb(INTERRUPT_ENABLE_REGISTER, 0x00);
        outb(LINE_CONTROL_REGISTER, 0x03);
        outb(FIFO_CONTROL_REGISTER, 0xC7);
        outb(MODEM_CONTROL_REGISTER, 0x0B);
    }
}

pub fn write_str(text: &str) {
    for byte in text.bytes() {
        if byte == b'\n' {
            write_byte(b'\r');
        }
        write_byte(byte);
    }
}

fn write_byte(byte: u8) {
    unsafe {
        while (inb(LINE_STATUS_REGISTER) & TRANSMIT_HOLDING_REGISTER_EMPTY) == 0 {}
        outb(DATA_REGISTER, byte);
    }
}

struct SerialWriter;

impl Write for SerialWriter {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        write_str(text);
        Ok(())
    }
}

pub fn write_fmt(args: core::fmt::Arguments<'_>) {
    let mut writer = SerialWriter;
    let _ = writer.write_fmt(args);
}


unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    value
}
