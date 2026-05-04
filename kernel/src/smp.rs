use alloc::vec::Vec;
use core::cell::UnsafeCell;

use crate::acpi::{MadtInfo, MadtProcessor};
use crate::arch;

struct GlobalTopology(UnsafeCell<Option<SmpTopology>>);

unsafe impl Sync for GlobalTopology {}

impl GlobalTopology {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<SmpTopology> {
        self.0.get()
    }
}

static TOPOLOGY: GlobalTopology = GlobalTopology::new();

#[derive(Clone)]
pub struct SmpTopology {
    pub bsp_apic_id: u32,
    pub current_cpu_index: usize,
    pub cpus: Vec<CpuTopologyEntry>,
}

impl SmpTopology {
    pub fn summary(&self) -> SmpSummary {
        let total_cpus = self.cpus.len();
        let enabled_cpus = self.cpus.iter().filter(|cpu| cpu.enabled).count();
        let online_cpus = self.cpus.iter().filter(|cpu| cpu.online).count();
        let bringup_targets = self
            .cpus
            .iter()
            .filter(|cpu| cpu.enabled && !cpu.online)
            .count();
        let x2apic_cpus = self.cpus.iter().filter(|cpu| cpu.x2apic).count();
        let ap_count = total_cpus.saturating_sub(1);

        SmpSummary {
            total_cpus,
            enabled_cpus,
            online_cpus,
            bringup_targets,
            x2apic_cpus,
            ap_count,
            bsp_apic_id: self.bsp_apic_id,
            current_cpu_index: self.current_cpu_index,
        }
    }

    pub fn current_cpu(&self) -> CpuTopologyEntry {
        self.cpus[self.current_cpu_index]
    }

    pub fn first_bringup_target(&self) -> Option<CpuTopologyEntry> {
        self.cpus
            .iter()
            .copied()
            .find(|cpu| cpu.enabled && !cpu.online)
    }
}

#[derive(Copy, Clone)]
pub struct CpuTopologyEntry {
    pub index: usize,
    pub processor_uid: u32,
    pub apic_id: u32,
    pub enabled: bool,
    pub online_capable: bool,
    pub x2apic: bool,
    pub is_bsp: bool,
    pub online: bool,
}

impl CpuTopologyEntry {
    pub fn apic_mode(self) -> &'static str {
        if self.x2apic { "x2apic" } else { "xapic" }
    }
}

#[derive(Copy, Clone)]
pub struct SmpSummary {
    pub total_cpus: usize,
    pub enabled_cpus: usize,
    pub online_cpus: usize,
    pub bringup_targets: usize,
    pub x2apic_cpus: usize,
    pub ap_count: usize,
    pub bsp_apic_id: u32,
    pub current_cpu_index: usize,
}

#[derive(Copy, Clone)]
pub enum SmpError {
    EmptyProcessorSet,
    MissingBspMatch,
}

impl SmpError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmptyProcessorSet => "madt does not expose any processors",
            Self::MissingBspMatch => "madt does not contain the current bsp apic id",
        }
    }
}

pub fn initialize(cpu_info: &arch::x86_64::CpuInfo, madt: &MadtInfo) -> Result<SmpSummary, SmpError> {
    if madt.processors.is_empty() {
        return Err(SmpError::EmptyProcessorSet);
    }

    let bsp_apic_id = cpu_info.initial_apic_id;
    let mut cpus = Vec::with_capacity(madt.processors.len());
    let mut current_cpu_index = None;

    for (index, processor) in madt.processors.iter().copied().enumerate() {
        let entry = build_cpu_entry(index, processor, bsp_apic_id, cpu_info.bootstrap_processor);
        if entry.is_bsp {
            current_cpu_index = Some(index);
        }
        cpus.push(entry);
    }

    let current_cpu_index = current_cpu_index.ok_or(SmpError::MissingBspMatch)?;
    let topology = SmpTopology {
        bsp_apic_id,
        current_cpu_index,
        cpus,
    };
    let summary = topology.summary();

    unsafe {
        *TOPOLOGY.get() = Some(topology);
    }

    Ok(summary)
}

pub fn topology() -> Option<&'static SmpTopology> {
    unsafe { (&*TOPOLOGY.get()).as_ref() }
}

pub fn mark_cpu_online(index: usize) {
    unsafe {
        if let Some(ref mut topology) = *TOPOLOGY.get() {
            if let Some(ref mut cpu) = topology.cpus.get_mut(index) {
                cpu.online = true;
            }
        }
    }
}

fn build_cpu_entry(
    index: usize,
    processor: MadtProcessor,
    bsp_apic_id: u32,
    bootstrap_processor: bool,
) -> CpuTopologyEntry {
    let is_bsp = bootstrap_processor && processor.apic_id == bsp_apic_id;
    CpuTopologyEntry {
        index,
        processor_uid: processor.processor_uid,
        apic_id: processor.apic_id,
        enabled: processor.enabled,
        online_capable: processor.online_capable,
        x2apic: processor.x2apic,
        is_bsp,
        online: is_bsp,
    }
}
