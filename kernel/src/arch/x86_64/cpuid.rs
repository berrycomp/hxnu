use core::arch::x86_64::{__cpuid, __cpuid_count, CpuidResult};

const VENDOR_INTEL: &[u8; 12] = b"GenuineIntel";
const VENDOR_AMD: &[u8; 12] = b"AuthenticAMD";
const CPUID_LEAF_EXTENDED_TOPOLOGY: u32 = 0x0b;
const CPUID_LEAF_EXTENDED_TOPOLOGY_V2: u32 = 0x1f;
const CPUID_TOPOLOGY_LEVEL_TYPE_INVALID: u8 = 0;
const CPUID_TOPOLOGY_LEVEL_TYPE_SMT: u8 = 1;
const CPUID_TOPOLOGY_LEVEL_TYPE_CORE: u8 = 2;
const MAX_TOPOLOGY_LEVELS: usize = 4;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum CpuVendor {
    Intel,
    Amd,
    Unknown,
}

impl CpuVendor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Intel => "intel",
            Self::Amd => "amd",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone)]
pub struct CpuIdentity {
    pub vendor: CpuVendor,
    pub vendor_id: [u8; 12],
    pub brand: [u8; 48],
    pub max_basic_leaf: u32,
    pub max_extended_leaf: u32,
    pub hypervisor_present: bool,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum TopologyLeafKind {
    Leaf0x0b,
    Leaf0x1f,
}

impl TopologyLeafKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Leaf0x0b => "0x0b",
            Self::Leaf0x1f => "0x1f",
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum TopologyLevelType {
    Invalid,
    Smt,
    Core,
    Unknown(u8),
}

impl TopologyLevelType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Invalid => "invalid",
            Self::Smt => "smt",
            Self::Core => "core",
            Self::Unknown(_) => "unknown",
        }
    }
}

#[derive(Copy, Clone)]
pub struct TopologyLevel {
    pub level_number: u8,
    pub level_type: TopologyLevelType,
    pub shift: u8,
    pub logical_processors: u16,
    pub x2apic_id: u32,
}

impl TopologyLevel {
    const fn empty() -> Self {
        Self {
            level_number: 0,
            level_type: TopologyLevelType::Invalid,
            shift: 0,
            logical_processors: 0,
            x2apic_id: 0,
        }
    }
}

#[derive(Copy, Clone)]
pub struct CpuTopologyInfo {
    pub leaf_kind: TopologyLeafKind,
    pub level_count: usize,
    pub levels: [TopologyLevel; MAX_TOPOLOGY_LEVELS],
    pub x2apic_id: u32,
    pub smt_shift: u8,
    pub core_shift: u8,
    pub threads_per_core: u16,
    pub logical_processors_per_package: u16,
    pub smt_id: u32,
    pub core_id: u32,
    pub package_id: u32,
}

pub fn query(leaf: u32) -> CpuidResult {
    __cpuid(leaf)
}

pub fn query_count(leaf: u32, subleaf: u32) -> CpuidResult {
    __cpuid_count(leaf, subleaf)
}

pub fn max_basic_leaf() -> u32 {
    query(0).eax
}

pub fn max_extended_leaf() -> u32 {
    query(0x8000_0000).eax
}

pub fn read_identity() -> CpuIdentity {
    let vendor_leaf = query(0);
    let max_basic_leaf = vendor_leaf.eax;
    let mut vendor_id = [0u8; 12];
    vendor_id[0..4].copy_from_slice(&vendor_leaf.ebx.to_le_bytes());
    vendor_id[4..8].copy_from_slice(&vendor_leaf.edx.to_le_bytes());
    vendor_id[8..12].copy_from_slice(&vendor_leaf.ecx.to_le_bytes());

    let vendor = match &vendor_id {
        VENDOR_INTEL => CpuVendor::Intel,
        VENDOR_AMD => CpuVendor::Amd,
        _ => CpuVendor::Unknown,
    };

    let max_extended_leaf = max_extended_leaf();
    let leaf_1 = query(1);
    let hypervisor_present = (leaf_1.ecx & (1 << 31)) != 0;

    let mut brand = [0u8; 48];
    if max_extended_leaf >= 0x8000_0004 {
        for (index, leaf) in [0x8000_0002, 0x8000_0003, 0x8000_0004]
            .iter()
            .copied()
            .enumerate()
        {
            let result = query(leaf);
            let offset = index * 16;
            brand[offset..offset + 4].copy_from_slice(&result.eax.to_le_bytes());
            brand[offset + 4..offset + 8].copy_from_slice(&result.ebx.to_le_bytes());
            brand[offset + 8..offset + 12].copy_from_slice(&result.ecx.to_le_bytes());
            brand[offset + 12..offset + 16].copy_from_slice(&result.edx.to_le_bytes());
        }
    }

    CpuIdentity {
        vendor,
        vendor_id,
        brand,
        max_basic_leaf,
        max_extended_leaf,
        hypervisor_present,
    }
}

pub fn read_topology(max_basic_leaf: u32) -> Option<CpuTopologyInfo> {
    if max_basic_leaf >= CPUID_LEAF_EXTENDED_TOPOLOGY_V2 {
        read_topology_leaf(CPUID_LEAF_EXTENDED_TOPOLOGY_V2, TopologyLeafKind::Leaf0x1f)
    } else if max_basic_leaf >= CPUID_LEAF_EXTENDED_TOPOLOGY {
        read_topology_leaf(CPUID_LEAF_EXTENDED_TOPOLOGY, TopologyLeafKind::Leaf0x0b)
    } else {
        None
    }
}

fn read_topology_leaf(leaf: u32, leaf_kind: TopologyLeafKind) -> Option<CpuTopologyInfo> {
    let mut levels = [TopologyLevel::empty(); MAX_TOPOLOGY_LEVELS];
    let mut level_count = 0usize;
    let mut x2apic_id = 0u32;
    let mut smt_shift = 0u8;
    let mut core_shift = 0u8;
    let mut threads_per_core = 1u16;
    let mut logical_processors_per_package = 1u16;

    for subleaf in 0..MAX_TOPOLOGY_LEVELS as u32 {
        let result = query_count(leaf, subleaf);
        let logical_processors = (result.ebx & 0xffff) as u16;
        let level_type_raw = ((result.ecx >> 8) & 0xff) as u8;
        if logical_processors == 0 || level_type_raw == CPUID_TOPOLOGY_LEVEL_TYPE_INVALID {
            break;
        }

        let level_type = parse_topology_level_type(level_type_raw);
        let shift = (result.eax & 0x1f) as u8;
        let level_number = (result.ecx & 0xff) as u8;
        x2apic_id = result.edx;

        levels[level_count] = TopologyLevel {
            level_number,
            level_type,
            shift,
            logical_processors,
            x2apic_id,
        };
        level_count += 1;

        match level_type {
            TopologyLevelType::Smt => {
                smt_shift = shift;
                threads_per_core = logical_processors.max(1);
            }
            TopologyLevelType::Core => {
                core_shift = shift;
                logical_processors_per_package = logical_processors.max(1);
            }
            TopologyLevelType::Invalid | TopologyLevelType::Unknown(_) => {}
        }
    }

    if level_count == 0 {
        return None;
    }

    if core_shift == 0 {
        core_shift = levels[level_count - 1].shift;
        logical_processors_per_package = levels[level_count - 1].logical_processors.max(1);
    }

    let smt_mask = bitmask(smt_shift);
    let core_width = core_shift.saturating_sub(smt_shift);
    let core_mask = bitmask(core_width);
    let smt_id = x2apic_id & smt_mask;
    let core_id = (x2apic_id >> smt_shift) & core_mask;
    let package_id = x2apic_id >> core_shift;

    Some(CpuTopologyInfo {
        leaf_kind,
        level_count,
        levels,
        x2apic_id,
        smt_shift,
        core_shift,
        threads_per_core,
        logical_processors_per_package,
        smt_id,
        core_id,
        package_id,
    })
}

fn parse_topology_level_type(raw: u8) -> TopologyLevelType {
    match raw {
        CPUID_TOPOLOGY_LEVEL_TYPE_INVALID => TopologyLevelType::Invalid,
        CPUID_TOPOLOGY_LEVEL_TYPE_SMT => TopologyLevelType::Smt,
        CPUID_TOPOLOGY_LEVEL_TYPE_CORE => TopologyLevelType::Core,
        value => TopologyLevelType::Unknown(value),
    }
}

fn bitmask(width: u8) -> u32 {
    if width >= 32 {
        u32::MAX
    } else if width == 0 {
        0
    } else {
        (1u32 << width) - 1
    }
}
