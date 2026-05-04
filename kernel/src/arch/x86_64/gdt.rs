use core::arch::asm;
use core::cell::UnsafeCell;
use core::mem::size_of;
use core::ptr;

const KERNEL_CODE_SELECTOR: u16 = 0x28;
const KERNEL_DATA_SELECTOR: u16 = 0x30;

pub const USER_CODE_SELECTOR: u16 = 0x3b;
pub const USER_DATA_SELECTOR: u16 = 0x43;
const TSS_SELECTOR: u16 = 0x48;

const GDT_ENTRY_COUNT: usize = 11;

struct GlobalGdt(UnsafeCell<[u64; GDT_ENTRY_COUNT]>);

unsafe impl Sync for GlobalGdt {}

impl GlobalGdt {
    const fn new() -> Self {
        Self(UnsafeCell::new([
            0x0000_0000_0000_0000, // 0x00: null
            0x0000_0000_0000_0000, // 0x08: null
            0x0000_0000_0000_0000, // 0x10: null
            0x0000_0000_0000_0000, // 0x18: null
            0x0000_0000_0000_0000, // 0x20: null
            // Pre-set the accessed bits so segment loads do not try to mutate read-only mappings.
            0x00af_9b00_0000_ffff, // 0x28: 64-bit code, DPL=0
            0x00cf_9300_0000_ffff, // 0x30: data, DPL=0
            0x00af_fb00_0000_ffff, // 0x38: 64-bit code, DPL=3
            0x00cf_f300_0000_ffff, // 0x40: data, DPL=3
            0x0000_0000_0000_0000, // 0x48: TSS low (patched at runtime)
            0x0000_0000_0000_0000, // 0x50: TSS high (patched at runtime)
        ]))
    }

    fn get(&self) -> *mut [u64; GDT_ENTRY_COUNT] {
        self.0.get()
    }
}

static GDT: GlobalGdt = GlobalGdt::new();

const INTERRUPT_STACK_PAGES: usize = 2;

#[repr(C, align(4096))]
struct InterruptStack {
    _bytes: [u8; INTERRUPT_STACK_PAGES * 4096],
}

static mut INTERRUPT_STACK: InterruptStack = InterruptStack {
    _bytes: [0; INTERRUPT_STACK_PAGES * 4096],
};

#[repr(C, packed)]
struct TaskStateSegment {
    reserved0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved1: u64,
    ist1: u64,
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    reserved2: u64,
    reserved3: u16,
    io_map_base: u16,
}

static mut TSS: TaskStateSegment = TaskStateSegment {
    reserved0: 0,
    rsp0: 0,
    rsp1: 0,
    rsp2: 0,
    reserved1: 0,
    ist1: 0,
    ist2: 0,
    ist3: 0,
    ist4: 0,
    ist5: 0,
    ist6: 0,
    ist7: 0,
    reserved2: 0,
    reserved3: 0,
    io_map_base: 0,
};

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
        limit: (size_of::<[u64; GDT_ENTRY_COUNT]>() - 1) as u16,
        base: GDT.get() as u64,
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

pub fn initialize() {
    load_table_only();
    reload_code_segment();
    reload_data_segments();
}

pub fn load_tss() {
    unsafe {
        let stack_base = ptr::addr_of!(INTERRUPT_STACK) as u64;
        let stack_top = stack_base + size_of::<InterruptStack>() as u64;
        TSS.rsp0 = stack_top;

        let tss_base = ptr::addr_of!(TSS) as u64;
        let limit = size_of::<TaskStateSegment>() - 1;

        let lower = (limit as u64 & 0xffff)
            | ((tss_base & 0xffff) << 16)
            | (((tss_base >> 16) & 0xff) << 32)
            | (0x89u64 << 40)
            | (((tss_base >> 24) & 0xff) << 56);

        let upper = tss_base >> 32;

        let gdt = GDT.get();
        (*gdt)[9] = lower;
        (*gdt)[10] = upper;

        load_table_only();

        asm!(
            "ltr {0:x}",
            in(reg) TSS_SELECTOR,
            options(nostack, preserves_flags),
        );
    }
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
