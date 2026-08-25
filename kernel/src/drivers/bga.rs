// THOL (Turkish Hybrid Open License)
// This file is strictly governed by the Turkish Hybrid Open License (THOL).

use core::arch::asm;

pub const VBE_DISPI_IOPORT_INDEX: u16 = 0x01CE;
pub const VBE_DISPI_IOPORT_DATA: u16 = 0x01CF;

pub const VBE_DISPI_INDEX_ID: u16 = 0x0;
pub const VBE_DISPI_INDEX_XRES: u16 = 0x1;
pub const VBE_DISPI_INDEX_YRES: u16 = 0x2;
pub const VBE_DISPI_INDEX_BPP: u16 = 0x3;
pub const VBE_DISPI_INDEX_ENABLE: u16 = 0x4;
pub const VBE_DISPI_INDEX_BANK: u16 = 0x5;
pub const VBE_DISPI_INDEX_VIRT_WIDTH: u16 = 0x6;
pub const VBE_DISPI_INDEX_VIRT_HEIGHT: u16 = 0x7;
pub const VBE_DISPI_INDEX_X_OFFSET: u16 = 0x8;
pub const VBE_DISPI_INDEX_Y_OFFSET: u16 = 0x9;

pub const VBE_DISPI_ID0: u16 = 0xB0C0;
pub const VBE_DISPI_ID1: u16 = 0xB0C1;
pub const VBE_DISPI_ID2: u16 = 0xB0C2;
pub const VBE_DISPI_ID3: u16 = 0xB0C3;
pub const VBE_DISPI_ID4: u16 = 0xB0C4;
pub const VBE_DISPI_ID5: u16 = 0xB0C5;

pub const VBE_DISPI_DISABLED: u16 = 0x00;
pub const VBE_DISPI_ENABLED: u16 = 0x01;
pub const VBE_DISPI_GETCAPS: u16 = 0x02;
pub const VBE_DISPI_8BIT_DAC: u16 = 0x20;
pub const VBE_DISPI_LFB_ENABLED: u16 = 0x40;
pub const VBE_DISPI_NOCLEARMEM: u16 = 0x80;

#[derive(Copy, Clone, Debug)]
pub struct BgaCaps {
    pub max_width: u16,
    pub max_height: u16,
    pub max_bpp: u16,
}

#[derive(Copy, Clone, Debug)]
pub struct BgaDisplayMode {
    pub width: u32,
    pub height: u32,
    pub bpp: u16,
    pub pitch: u32,
}

unsafe fn outw(port: u16, value: u16) {
    unsafe {
        asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack, preserves_flags));
    }
}

unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    unsafe {
        asm!("in ax, dx", out("ax") value, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    value
}

pub fn bga_write(index: u16, value: u16) {
    unsafe {
        outw(VBE_DISPI_IOPORT_INDEX, index);
        outw(VBE_DISPI_IOPORT_DATA, value);
    }
}

pub fn bga_read(index: u16) -> u16 {
    unsafe {
        outw(VBE_DISPI_IOPORT_INDEX, index);
        inw(VBE_DISPI_IOPORT_DATA)
    }
}

pub fn is_available() -> bool {
    bga_write(VBE_DISPI_INDEX_ID, VBE_DISPI_ID5);
    let id = bga_read(VBE_DISPI_INDEX_ID);
    id >= VBE_DISPI_ID0 && id <= VBE_DISPI_ID5
}

pub fn probe_caps() -> BgaCaps {
    bga_write(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_GETCAPS);
    let max_width = bga_read(VBE_DISPI_INDEX_XRES);
    let max_height = bga_read(VBE_DISPI_INDEX_YRES);
    let max_bpp = bga_read(VBE_DISPI_INDEX_BPP);
    BgaCaps {
        max_width: if max_width == 0 { 1920 } else { max_width },
        max_height: if max_height == 0 { 1080 } else { max_height },
        max_bpp: if max_bpp == 0 { 32 } else { max_bpp },
    }
}

pub fn set_mode(width: u32, height: u32, bpp: u16) -> BgaDisplayMode {
    bga_write(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_DISABLED);
    bga_write(VBE_DISPI_INDEX_XRES, width as u16);
    bga_write(VBE_DISPI_INDEX_YRES, height as u16);
    bga_write(VBE_DISPI_INDEX_BPP, bpp);
    bga_write(
        VBE_DISPI_INDEX_ENABLE,
        VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED,
    );
    let pitch = width * (bpp as u32 / 8);
    BgaDisplayMode {
        width,
        height,
        bpp,
        pitch,
    }
}
