use core::arch::asm;
use core::cell::UnsafeCell;
use core::mem::size_of;

const KERNEL_CODE_SELECTOR: u16 = 0x28;
const KERNEL_DATA_SELECTOR: u16 = 0x30;
pub const USER_CODE_SELECTOR: u16 = 0x38;
pub const USER_DATA_SELECTOR: u16 = 0x40;
const TSS_SELECTOR: u16 = 0x48;

const GDT_ENTRY_COUNT: usize = 11;

const KERNEL_CODE_INDEX: usize = 5;
const KERNEL_DATA_INDEX: usize = 6;
const USER_CODE_INDEX: usize = 7;
const USER_DATA_INDEX: usize = 8;
const TSS_LOW_INDEX: usize = 9;
const TSS_HIGH_INDEX: usize = 10;

struct GlobalGdt(UnsafeCell<[u64; GDT_ENTRY_COUNT]>);

unsafe impl Sync for GlobalGdt {}

static GDT: GlobalGdt = GlobalGdt(UnsafeCell::new([0; GDT_ENTRY_COUNT]));

#[repr(C, packed)]
struct TaskStateSegment {
    _reserved0: u32,
    pub rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    _reserved1: u64,
    ist1: u64,
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    _reserved2: u64,
    _reserved3: u16,
    io_map_base: u16,
}

struct GlobalTss(UnsafeCell<TaskStateSegment>);

unsafe impl Sync for GlobalTss {}

static TSS: GlobalTss = GlobalTss(UnsafeCell::new(TaskStateSegment {
    _reserved0: 0,
    rsp0: 0,
    rsp1: 0,
    rsp2: 0,
    _reserved1: 0,
    ist1: 0,
    ist2: 0,
    ist3: 0,
    ist4: 0,
    ist5: 0,
    ist6: 0,
    ist7: 0,
    _reserved2: 0,
    _reserved3: 0,
    io_map_base: 0,
}));

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

#[derive(Copy, Clone)]
pub struct SegmentSelectors {
    pub cs: u16,
    pub ds: u16,
    pub es: u16,
    pub fs: u16,
    pub gs: u16,
    pub ss: u16,
}

pub fn set_tss_rsp0(rsp0: u64) {
    unsafe {
        (*TSS.0.get()).rsp0 = rsp0;
    }
}

pub fn read_segment_selectors() -> SegmentSelectors {
    let cs = read_cs();
    let ds = read_ds();
    let es = read_es();
    let fs = read_fs();
    let gs = read_gs();
    let ss = read_ss();

    SegmentSelectors { cs, ds, es, fs, gs, ss }
}

pub fn load_table_only() {
    let gdtr = DescriptorTablePointer {
        limit: ((GDT_ENTRY_COUNT * size_of::<u64>()) - 1) as u16,
        base: (unsafe { (*GDT.0.get()).as_ptr() }) as u64,
    };

    unsafe {
        asm!(
            "lgdt [{gdtr}]",
            gdtr = in(reg) &gdtr,
            options(readonly, nostack, preserves_flags),
        );
    }
}

pub fn reload_code_segment() {
    unsafe {
        asm!(
            "mov rdx, {code_selector}",
            "push rdx",
            "lea rax, [rip + 2f]",
            "push rax",
            "retfq",
            "2:",
            code_selector = in(reg) (KERNEL_CODE_SELECTOR as u64),
            lateout("rax") _,
            lateout("rdx") _,
            options(preserves_flags),
        );
    }
}

pub fn reload_data_segments() {
    reload_ds();
    reload_es();
    reload_fs();
    reload_gs();
    reload_ss();
}

pub fn reload_ds() {
    unsafe {
        asm!(
            "mov rdx, {data_selector}",
            "mov ax, dx",
            "mov ds, ax",
            data_selector = in(reg) (KERNEL_DATA_SELECTOR as u64),
            lateout("rdx") _,
            options(preserves_flags),
        );
    }
}

pub fn reload_es() {
    unsafe {
        asm!(
            "mov rdx, {data_selector}",
            "mov ax, dx",
            "mov es, ax",
            data_selector = in(reg) (KERNEL_DATA_SELECTOR as u64),
            lateout("rdx") _,
            options(preserves_flags),
        );
    }
}

pub fn reload_fs() {
    unsafe {
        asm!(
            "mov rdx, {data_selector}",
            "mov ax, dx",
            "mov fs, ax",
            data_selector = in(reg) (KERNEL_DATA_SELECTOR as u64),
            lateout("rdx") _,
            options(preserves_flags),
        );
    }
}

pub fn reload_gs() {
    unsafe {
        asm!(
            "mov rdx, {data_selector}",
            "mov ax, dx",
            "mov gs, ax",
            data_selector = in(reg) (KERNEL_DATA_SELECTOR as u64),
            lateout("rdx") _,
            options(preserves_flags),
        );
    }
}

pub fn reload_ss() {
    unsafe {
        asm!(
            "mov rdx, {data_selector}",
            "mov ax, dx",
            "mov ss, ax",
            data_selector = in(reg) (KERNEL_DATA_SELECTOR as u64),
            lateout("rdx") _,
            options(preserves_flags),
        );
    }
}

fn load_task_register() {
    unsafe {
        asm!(
            "ltr {selector:x}",
            selector = in(reg) TSS_SELECTOR,
            options(nostack, preserves_flags),
        );
    }
}

pub fn initialize() {
    unsafe {
        let gdt = &mut *GDT.0.get();
        gdt[KERNEL_CODE_INDEX] = 0x00af_9b00_0000_ffff;
        gdt[KERNEL_DATA_INDEX] = 0x00cf_9300_0000_ffff;
        gdt[USER_CODE_INDEX] = 0x00af_fb00_0000_ffff;
        gdt[USER_DATA_INDEX] = 0x00cf_f300_0000_ffff;

        let tss_ptr = TSS.0.get() as u64;
        let tss_limit = size_of::<TaskStateSegment>() as u64 - 1;
        gdt[TSS_LOW_INDEX] = make_tss_low(tss_ptr, tss_limit);
        gdt[TSS_HIGH_INDEX] = make_tss_high(tss_ptr);
    }

    load_table_only();
    reload_code_segment();
    reload_data_segments();
    load_task_register();
}

fn make_tss_low(base: u64, limit: u64) -> u64 {
    let base_lo = base & 0xFFFFFF;
    let base_mid = (base >> 24) & 0xFF;
    let limit_lo = limit & 0xFFFF;
    let limit_hi = (limit >> 16) & 0xF;
    limit_lo
        | (base_lo << 16)
        | (0x89u64 << 40)
        | (limit_hi << 48)
        | (base_mid << 56)
}

fn make_tss_high(base: u64) -> u64 {
    base >> 32
}

fn read_cs() -> u16 {
    let value: u16;
    unsafe {
        asm!("mov {segment:x}, cs", segment = out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn read_ds() -> u16 {
    let value: u16;
    unsafe {
        asm!("mov {segment:x}, ds", segment = out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn read_es() -> u16 {
    let value: u16;
    unsafe {
        asm!("mov {segment:x}, es", segment = out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn read_fs() -> u16 {
    let value: u16;
    unsafe {
        asm!("mov {segment:x}, fs", segment = out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn read_gs() -> u16 {
    let value: u16;
    unsafe {
        asm!("mov {segment:x}, gs", segment = out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn read_ss() -> u16 {
    let value: u16;
    unsafe {
        asm!("mov {segment:x}, ss", segment = out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}
