use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::kprintln;
use crate::time;

use super::cpu::CpuInfo;
use super::early_map;

pub const TIMER_VECTOR: usize = 0x20;
pub const SPURIOUS_VECTOR: usize = 0xff;

const PIC_MASTER_DATA: u16 = 0x21;
const PIC_SLAVE_DATA: u16 = 0xa1;

const APIC_TPR: u32 = 0x080;
const APIC_EOI: u32 = 0x0b0;
const APIC_SVR: u32 = 0x0f0;
const APIC_ICR_LOW: u32 = 0x300;
const APIC_ICR_HIGH: u32 = 0x310;
const APIC_LVT_TIMER: u32 = 0x320;
const APIC_TIMER_INITIAL_COUNT: u32 = 0x380;
const APIC_TIMER_DIVIDE: u32 = 0x3e0;

const APIC_SVR_ENABLE: u32 = 1 << 8;
const APIC_LVT_MASKED: u32 = 1 << 16;
const APIC_LVT_TIMER_PERIODIC: u32 = 1 << 17;
const APIC_TIMER_DIVIDE_BY_16: u32 = 0x3;
const APIC_TIMER_DIVIDE_VALUE: u32 = 16;
const APIC_TIMER_INITIAL_TICKS: u32 = 1_000_000;
const APIC_PERIODIC_INITIAL_TICKS: u32 = 250_000;
const APIC_TIMER_TIMEOUT_NS: u64 = 500_000_000;

const ICR_DELIVERY_STATUS: u32 = 1 << 12;
const ICR_LEVEL_ASSERT: u32 = 1 << 14;
const ICR_TRIGGER_LEVEL: u32 = 1 << 15;
const ICR_DELIVERY_INIT: u32 = 5 << 8;
const ICR_DELIVERY_STARTUP: u32 = 6 << 8;

static APIC_BASE_VIRTUAL: AtomicU64 = AtomicU64::new(0);
static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);
static TIMER_LOG_BUDGET: AtomicU64 = AtomicU64::new(0);
static SPURIOUS_REPORTED: AtomicBool = AtomicBool::new(false);

#[derive(Copy, Clone)]
pub struct TimerBringUp {
    pub vector: u8,
    pub divide_value: u32,
    pub initial_count: u32,
    pub ticks_observed: u64,
}

#[derive(Copy, Clone)]
pub struct PeriodicTimer {
    pub vector: u8,
    pub divide_value: u32,
    pub initial_count: u32,
}

#[derive(Copy, Clone, Debug)]
pub enum TimerError {
    Unsupported,
    X2ApicModeUnsupported,
    GlobalEnableMissing,
    MissingBaseAddress,
    PageTableAllocationFailed,
    Timeout,
}

impl TimerError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "local APIC is not supported",
            Self::X2ApicModeUnsupported => "x2APIC mode is active but not supported yet",
            Self::GlobalEnableMissing => "local APIC is not globally enabled",
            Self::MissingBaseAddress => "local APIC base address is missing",
            Self::PageTableAllocationFailed => "failed to allocate a page-table page for APIC MMIO",
            Self::Timeout => "timer interrupt did not arrive before timeout",
        }
    }
}

impl From<early_map::MapError> for TimerError {
    fn from(value: early_map::MapError) -> Self {
        match value {
            early_map::MapError::AddressOverflow => Self::MissingBaseAddress,
            early_map::MapError::PageTableAllocationFailed => Self::PageTableAllocationFailed,
            early_map::MapError::MappingConflict => Self::MissingBaseAddress,
        }
    }
}

pub fn initialize_timer(hhdm_offset: u64, cpu_info: &CpuInfo) -> Result<TimerBringUp, TimerError> {
    ensure_timer_ready(hhdm_offset, cpu_info)?;

    TIMER_TICKS.store(0, Ordering::Release);
    TIMER_LOG_BUDGET.store(1, Ordering::Release);

    write_register(APIC_LVT_TIMER, APIC_LVT_MASKED | (TIMER_VECTOR as u32));
    write_register(APIC_TIMER_DIVIDE, APIC_TIMER_DIVIDE_BY_16);
    write_register(APIC_TIMER_INITIAL_COUNT, 0);

    // Start with a single interrupt so the self-test can confirm delivery without flooding logs.
    write_register(APIC_LVT_TIMER, TIMER_VECTOR as u32);
    write_register(APIC_TIMER_INITIAL_COUNT, APIC_TIMER_INITIAL_TICKS);

    let deadline = time::uptime_nanoseconds().saturating_add(APIC_TIMER_TIMEOUT_NS);
    enable_interrupts();
    while TIMER_TICKS.load(Ordering::Acquire) == 0 {
        if time::uptime_nanoseconds() >= deadline {
            disable_interrupts();
            mask_timer();
            return Err(TimerError::Timeout);
        }

        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }
    disable_interrupts();

    mask_timer();

    Ok(TimerBringUp {
        vector: TIMER_VECTOR as u8,
        divide_value: APIC_TIMER_DIVIDE_VALUE,
        initial_count: APIC_TIMER_INITIAL_TICKS,
        ticks_observed: TIMER_TICKS.load(Ordering::Acquire),
    })
}

pub fn start_periodic_timer(
    hhdm_offset: u64,
    cpu_info: &CpuInfo,
) -> Result<PeriodicTimer, TimerError> {
    ensure_timer_ready(hhdm_offset, cpu_info)?;

    TIMER_TICKS.store(0, Ordering::Release);
    TIMER_LOG_BUDGET.store(0, Ordering::Release);
    write_register(APIC_LVT_TIMER, APIC_LVT_MASKED | (TIMER_VECTOR as u32));
    write_register(APIC_TIMER_DIVIDE, APIC_TIMER_DIVIDE_BY_16);
    write_register(
        APIC_LVT_TIMER,
        APIC_LVT_TIMER_PERIODIC | (TIMER_VECTOR as u32),
    );
    write_register(APIC_TIMER_INITIAL_COUNT, APIC_PERIODIC_INITIAL_TICKS);

    Ok(PeriodicTimer {
        vector: TIMER_VECTOR as u8,
        divide_value: APIC_TIMER_DIVIDE_VALUE,
        initial_count: APIC_PERIODIC_INITIAL_TICKS,
    })
}

pub fn mask_periodic_timer() {
    mask_timer();
}

pub fn handle_timer_interrupt() -> u64 {
    let tick = TIMER_TICKS.fetch_add(1, Ordering::AcqRel) + 1;
    let remaining_logs = TIMER_LOG_BUDGET.load(Ordering::Acquire);
    if remaining_logs != 0
        && TIMER_LOG_BUDGET
            .compare_exchange(
                remaining_logs,
                remaining_logs - 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    {
        kprintln!("HXNU: apic timer interrupt tick={}", tick);
    }
    end_of_interrupt();
    tick
}

fn ensure_timer_ready(hhdm_offset: u64, cpu_info: &CpuInfo) -> Result<(), TimerError> {
    if !cpu_info.local_apic_supported {
        return Err(TimerError::Unsupported);
    }
    if cpu_info.x2apic_enabled {
        return Err(TimerError::X2ApicModeUnsupported);
    }
    if !cpu_info.apic_global_enabled {
        return Err(TimerError::GlobalEnableMissing);
    }
    if cpu_info.apic_base == 0 {
        return Err(TimerError::MissingBaseAddress);
    }

    ensure_apic_mapped(hhdm_offset, cpu_info.apic_base)?;
    enable_svr();
    Ok(())
}

pub fn handle_spurious_interrupt() {
    if !SPURIOUS_REPORTED.swap(true, Ordering::AcqRel) {
        kprintln!("HXNU: apic spurious interrupt");
    }
}

fn mask_timer() {
    write_register(APIC_LVT_TIMER, APIC_LVT_MASKED | (TIMER_VECTOR as u32));
    write_register(APIC_TIMER_INITIAL_COUNT, 0);
}

fn end_of_interrupt() {
    if APIC_BASE_VIRTUAL.load(Ordering::Acquire) != 0 {
        write_register(APIC_EOI, 0);
    }
}

fn enable_interrupts() {
    unsafe {
        asm!("sti", options(nomem, nostack, preserves_flags));
    }
}

fn disable_interrupts() {
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
    }
}

pub fn ensure_apic_mapped(hhdm_offset: u64, apic_base: u64) -> Result<u64, TimerError> {
    let apic_base_virtual = early_map::ensure_region_mapped(
        hhdm_offset,
        apic_base,
        4096,
        early_map::FLAG_WRITE_THROUGH | early_map::FLAG_CACHE_DISABLE,
    )?;
    APIC_BASE_VIRTUAL.store(apic_base_virtual, Ordering::Release);
    Ok(apic_base_virtual)
}

pub fn enable_svr() {
    mask_legacy_pic();
    write_register(APIC_TPR, 0);
    let spurious_value =
        (read_register(APIC_SVR) & !0xff) | APIC_SVR_ENABLE | (SPURIOUS_VECTOR as u32);
    write_register(APIC_SVR, spurious_value);
}

pub fn send_init_ipi(apic_id: u32) {
    wait_for_icr_idle();
    write_register(APIC_ICR_HIGH, apic_id << 24);
    write_register(
        APIC_ICR_LOW,
        ICR_DELIVERY_INIT | ICR_LEVEL_ASSERT | ICR_TRIGGER_LEVEL,
    );
}

pub fn send_init_ipi_deassert(apic_id: u32) {
    wait_for_icr_idle();
    write_register(APIC_ICR_HIGH, apic_id << 24);
    write_register(
        APIC_ICR_LOW,
        ICR_DELIVERY_INIT | ICR_TRIGGER_LEVEL,
    );
}

pub fn send_startup_ipi(apic_id: u32, vector: u8) {
    wait_for_icr_idle();
    let icr_low = ICR_DELIVERY_STARTUP | (vector as u32);
    crate::serial::write_str("HXNU: apic sipi enter\n");
    write_register(APIC_ICR_HIGH, apic_id << 24);
    write_register(APIC_ICR_LOW, icr_low);
    crate::serial::write_str("HXNU: apic sipi sent\n");
}

fn wait_for_icr_idle() {
    while read_register(APIC_ICR_LOW) & ICR_DELIVERY_STATUS != 0 {
        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }
}

fn mask_legacy_pic() {
    unsafe {
        outb(PIC_MASTER_DATA, 0xff);
        outb(PIC_SLAVE_DATA, 0xff);
    }
}

fn read_register(offset: u32) -> u32 {
    let register = register_ptr(offset);
    unsafe { read_volatile(register) }
}

fn write_register(offset: u32, value: u32) {
    let register = register_ptr(offset);
    unsafe {
        write_volatile(register, value);
    }
}

fn register_ptr(offset: u32) -> *mut u32 {
    let base = APIC_BASE_VIRTUAL.load(Ordering::Acquire);
    debug_assert_ne!(base, 0);
    (base.wrapping_add(offset as u64)) as *mut u32
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags),
        );
    }
}
