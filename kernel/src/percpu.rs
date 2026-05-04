use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::fmt::Write;
use core::mem::size_of;
use core::ptr;

use crate::arch;
use crate::limine;
use crate::mm;
use crate::smp;

const PERCPU_AREA_BYTES: usize = mm::frame::PAGE_SIZE as usize;
const PERCPU_MAGIC: u32 = 0x5043_5055;
const PERCPU_VERSION: u16 = 1;
const PERCPU_FLAG_BSP: u16 = 1 << 0;
const PERCPU_FLAG_ONLINE: u16 = 1 << 1;

struct GlobalPerCpu(UnsafeCell<Option<PerCpuState>>);

unsafe impl Sync for GlobalPerCpu {}

impl GlobalPerCpu {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<PerCpuState> {
        self.0.get()
    }
}

static PERCPU: GlobalPerCpu = GlobalPerCpu::new();

struct PerCpuState {
    current_cpu_index: usize,
    current_apic_id: u32,
    entries: Vec<PerCpuEntry>,
}

#[derive(Copy, Clone)]
pub struct PerCpuEntry {
    pub index: usize,
    pub apic_id: u32,
    pub processor_uid: u32,
    pub is_bsp: bool,
    pub online: bool,
    pub enabled: bool,
    pub physical_base: u64,
    pub virtual_base: u64,
    pub area_bytes: usize,
}

#[derive(Copy, Clone)]
pub struct PerCpuSummary {
    pub cpu_count: usize,
    pub online_cpus: usize,
    pub current_cpu_index: usize,
    pub current_apic_id: u32,
    pub area_bytes: usize,
    pub total_bytes: usize,
}

#[derive(Copy, Clone)]
pub enum PerCpuError {
    AlreadyInitialized,
    MissingTopology,
    MissingHhdm,
    MissingCurrentCpu,
    FrameAllocationFailed,
}

impl PerCpuError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "per-cpu state is already initialized",
            Self::MissingTopology => "smp topology is not initialized",
            Self::MissingHhdm => "limine HHDM offset is not available",
            Self::MissingCurrentCpu => "current apic id is not present in smp topology",
            Self::FrameAllocationFailed => "failed to allocate bootstrap per-cpu frame",
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct PerCpuAreaHeader {
    magic: u32,
    version: u16,
    flags: u16,
    cpu_index: u32,
    apic_id: u32,
    processor_uid: u32,
    _reserved: u32,
    self_virtual_base: u64,
    self_physical_base: u64,
}

pub fn initialize() -> Result<PerCpuSummary, PerCpuError> {
    let slot = unsafe { &mut *PERCPU.get() };
    if slot.is_some() {
        return Err(PerCpuError::AlreadyInitialized);
    }

    let topology = smp::topology().ok_or(PerCpuError::MissingTopology)?;
    let hhdm_offset = limine::hhdm_offset().ok_or(PerCpuError::MissingHhdm)?;
    let current_apic_id = arch::x86_64::current_initial_apic_id();
    let current_cpu_index = topology
        .cpus
        .iter()
        .position(|cpu| cpu.apic_id == current_apic_id)
        .ok_or(PerCpuError::MissingCurrentCpu)?;

    let mut entries = Vec::with_capacity(topology.cpus.len());
    for cpu in &topology.cpus {
        let frame = mm::frame::allocate_frame().ok_or(PerCpuError::FrameAllocationFailed)?;
        let physical_base = frame.start_address();
        let virtual_base = hhdm_offset
            .checked_add(physical_base)
            .ok_or(PerCpuError::MissingHhdm)?;

        unsafe {
            ptr::write_bytes(virtual_base as *mut u8, 0, PERCPU_AREA_BYTES);
            ptr::write(
                virtual_base as *mut PerCpuAreaHeader,
                PerCpuAreaHeader {
                    magic: PERCPU_MAGIC,
                    version: PERCPU_VERSION,
                    flags: cpu_flag_bits(*cpu),
                    cpu_index: cpu.index as u32,
                    apic_id: cpu.apic_id,
                    processor_uid: cpu.processor_uid,
                    _reserved: 0,
                    self_virtual_base: virtual_base,
                    self_physical_base: physical_base,
                },
            );
        }

        entries.push(PerCpuEntry {
            index: cpu.index,
            apic_id: cpu.apic_id,
            processor_uid: cpu.processor_uid,
            is_bsp: cpu.is_bsp,
            online: cpu.online,
            enabled: cpu.enabled,
            physical_base,
            virtual_base,
            area_bytes: PERCPU_AREA_BYTES,
        });
    }

    let state = PerCpuState {
        current_cpu_index,
        current_apic_id,
        entries,
    };
    let summary = state.summary();
    *slot = Some(state);
    Ok(summary)
}

pub fn summary() -> Option<PerCpuSummary> {
    Some(state_ref()?.summary())
}

pub fn render_status() -> String {
    let mut text = String::new();
    let Some(summary) = summary() else {
        let _ = writeln!(text, "initialized no");
        return text;
    };
    let state = state_ref().expect("percpu summary implies state");

    let _ = writeln!(
        text,
        "initialized yes cpus={} online={} current_index={} current_apic_id={} area_bytes={} total_bytes={}",
        summary.cpu_count,
        summary.online_cpus,
        summary.current_cpu_index,
        summary.current_apic_id,
        summary.area_bytes,
        summary.total_bytes,
    );
    let _ = writeln!(text, "cpus {}", summary.cpu_count);
    let _ = writeln!(text, "online {}", summary.online_cpus);
    let _ = writeln!(text, "current_index {}", summary.current_cpu_index);
    let _ = writeln!(text, "current_apic_id {}", summary.current_apic_id);
    let _ = writeln!(text, "area_bytes {}", summary.area_bytes);
    let _ = writeln!(text, "total_bytes {}", summary.total_bytes);

    for cpu in &state.entries {
        let _ = writeln!(
            text,
            "cpu{} apic={} uid={} bsp={} online={} enabled={} phys={:#018x} virt={:#018x} bytes={}",
            cpu.index,
            cpu.apic_id,
            cpu.processor_uid,
            yes_no(cpu.is_bsp),
            yes_no(cpu.online),
            yes_no(cpu.enabled),
            cpu.physical_base,
            cpu.virtual_base,
            cpu.area_bytes,
        );
    }

    text
}

pub fn entry_virtual_base(index: usize) -> Option<u64> {
    let state = state_ref()?;
    state.entries.get(index).map(|e| e.virtual_base)
}

pub fn header_size() -> usize {
    size_of::<PerCpuAreaHeader>()
}

fn state_ref() -> Option<&'static PerCpuState> {
    unsafe { (&*PERCPU.get()).as_ref() }
}

impl PerCpuState {
    fn summary(&self) -> PerCpuSummary {
        let cpu_count = self.entries.len();
        let online_cpus = self.entries.iter().filter(|cpu| cpu.online).count();
        let area_bytes = self.entries.first().map_or(0, |cpu| cpu.area_bytes);
        let total_bytes = area_bytes.saturating_mul(cpu_count);
        PerCpuSummary {
            cpu_count,
            online_cpus,
            current_cpu_index: self.current_cpu_index,
            current_apic_id: self.current_apic_id,
            area_bytes,
            total_bytes,
        }
    }
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

const fn cpu_flag_bits(cpu: smp::CpuTopologyEntry) -> u16 {
    let mut flags = 0u16;
    if cpu.is_bsp {
        flags |= PERCPU_FLAG_BSP;
    }
    if cpu.online {
        flags |= PERCPU_FLAG_ONLINE;
    }
    flags
}
