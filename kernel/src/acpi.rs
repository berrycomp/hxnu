use alloc::vec::Vec;
use core::mem::size_of;
use core::ptr;
use core::slice;
use core::str;

use crate::arch;

const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";
const SIGNATURE_RSDT: &[u8; 4] = b"RSDT";
const SIGNATURE_XSDT: &[u8; 4] = b"XSDT";
const SIGNATURE_MADT: &[u8; 4] = b"APIC";
const SIGNATURE_FADT: &[u8; 4] = b"FACP";

const MADT_ENTRY_LOCAL_APIC: u8 = 0;
const MADT_ENTRY_IO_APIC: u8 = 1;
const MADT_ENTRY_INTERRUPT_SOURCE_OVERRIDE: u8 = 2;
const MADT_ENTRY_LOCAL_APIC_ADDRESS_OVERRIDE: u8 = 5;
const MADT_ENTRY_LOCAL_X2APIC: u8 = 9;

const MADT_LOCAL_APIC_FLAG_ENABLED: u32 = 1 << 0;
const MADT_LOCAL_APIC_FLAG_ONLINE_CAPABLE: u32 = 1 << 1;

const FADT_FLAGS_RESET_REG_SUPPORTED: u32 = 1 << 10;
const FADT_FLAGS_HW_REDUCED_ACPI: u32 = 1 << 20;

const FADT_OFFSET_PREFERRED_PM_PROFILE: usize = 45;
const FADT_OFFSET_SCI_INTERRUPT: usize = 46;
const FADT_OFFSET_SMI_COMMAND_PORT: usize = 48;
const FADT_OFFSET_ACPI_ENABLE: usize = 52;
const FADT_OFFSET_ACPI_DISABLE: usize = 53;
const FADT_OFFSET_PM1A_CONTROL_BLOCK: usize = 64;
const FADT_OFFSET_PM1B_CONTROL_BLOCK: usize = 68;
const FADT_OFFSET_BOOT_ARCH_FLAGS: usize = 109;
const FADT_OFFSET_FLAGS: usize = 112;
const FADT_OFFSET_RESET_REGISTER: usize = 116;
const FADT_OFFSET_RESET_VALUE: usize = 128;

#[derive(Clone)]
pub struct AcpiDiscovery {
    pub revision: u8,
    pub oem_id: [u8; 6],
    pub rsdp_address: u64,
    pub root_address: u64,
    pub root_kind: RootKind,
    pub table_count: usize,
    pub valid_table_count: usize,
    pub invalid_table_count: usize,
    pub madt: Option<MadtInfo>,
    pub fadt: Option<FadtInfo>,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum RootKind {
    Rsdt,
    Xsdt,
}

impl RootKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rsdt => "RSDT",
            Self::Xsdt => "XSDT",
        }
    }
}

#[derive(Clone)]
pub struct MadtInfo {
    pub local_apic_address: u64,
    pub flags: u32,
    pub processors: Vec<MadtProcessor>,
    pub io_apics: Vec<IoApicInfo>,
    pub interrupt_source_overrides: Vec<InterruptSourceOverride>,
}

impl MadtInfo {
    pub fn total_processor_count(&self) -> usize {
        self.processors.len()
    }

    pub fn enabled_processor_count(&self) -> usize {
        self.processors.iter().filter(|processor| processor.enabled).count()
    }

    pub fn local_x2apic_count(&self) -> usize {
        self.processors.iter().filter(|processor| processor.x2apic).count()
    }
}

#[derive(Copy, Clone)]
pub struct MadtProcessor {
    pub processor_uid: u32,
    pub apic_id: u32,
    pub enabled: bool,
    pub online_capable: bool,
    pub x2apic: bool,
}

impl MadtProcessor {
    pub fn apic_mode(self) -> &'static str {
        if self.x2apic { "x2apic" } else { "xapic" }
    }
}

#[derive(Copy, Clone)]
pub struct IoApicInfo {
    pub io_apic_id: u8,
    pub address: u32,
    pub global_system_interrupt_base: u32,
}

#[derive(Copy, Clone)]
pub struct InterruptSourceOverride {
    pub source: u8,
    pub global_system_interrupt: u32,
    pub flags: u16,
}

#[derive(Copy, Clone)]
pub struct FadtInfo {
    pub revision: u8,
    pub length: u32,
    pub preferred_pm_profile: PowerProfile,
    pub sci_interrupt: u16,
    pub smi_command_port: u32,
    pub acpi_enable: u8,
    pub acpi_disable: u8,
    pub pm1a_control_block: u32,
    pub pm1b_control_block: u32,
    pub boot_architecture_flags: u16,
    pub flags: u32,
    pub reset_register: Option<GenericAddress>,
    pub reset_value: u8,
}

impl FadtInfo {
    pub fn reset_supported(self) -> bool {
        self.flags & FADT_FLAGS_RESET_REG_SUPPORTED != 0
            && self.reset_register.is_some()
            && self.reset_value != 0
    }

    pub fn hardware_reduced(self) -> bool {
        self.flags & FADT_FLAGS_HW_REDUCED_ACPI != 0
    }
}

#[derive(Copy, Clone)]
pub enum PowerProfile {
    Unspecified,
    Desktop,
    Mobile,
    Workstation,
    EnterpriseServer,
    SohoServer,
    AppliancePc,
    PerformanceServer,
    Tablet,
    Unknown,
}

impl PowerProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::Desktop => "desktop",
            Self::Mobile => "mobile",
            Self::Workstation => "workstation",
            Self::EnterpriseServer => "enterprise-server",
            Self::SohoServer => "soho-server",
            Self::AppliancePc => "appliance-pc",
            Self::PerformanceServer => "performance-server",
            Self::Tablet => "tablet",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Copy, Clone)]
pub struct GenericAddress {
    pub address_space: u8,
    pub bit_width: u8,
    pub bit_offset: u8,
    pub access_size: u8,
    pub address: u64,
}

impl GenericAddress {
    pub fn address_space_str(self) -> &'static str {
        match self.address_space {
            0 => "system-memory",
            1 => "system-io",
            2 => "pci-config",
            3 => "embedded-controller",
            4 => "smbus",
            0x0a => "platform-comm",
            0x7f => "functional-fixed",
            _ => "unknown",
        }
    }
}

#[derive(Copy, Clone)]
pub enum AcpiError {
    AddressOverflow,
    MappingFailed,
    InvalidRsdpSignature,
    InvalidRsdpChecksum,
    InvalidRsdpRevision,
    MissingRootTable,
    InvalidRootSignature,
    InvalidRootLength,
    InvalidRootChecksum,
}

impl AcpiError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AddressOverflow => "physical-to-virtual address overflow",
            Self::MappingFailed => "failed to map an acpi table page",
            Self::InvalidRsdpSignature => "invalid rsdp signature",
            Self::InvalidRsdpChecksum => "invalid rsdp checksum",
            Self::InvalidRsdpRevision => "invalid rsdp revision or length",
            Self::MissingRootTable => "missing acpi root table",
            Self::InvalidRootSignature => "invalid acpi root table signature",
            Self::InvalidRootLength => "invalid acpi root table length",
            Self::InvalidRootChecksum => "invalid acpi root table checksum",
        }
    }
}

#[repr(C)]
struct RsdpDescriptor {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_address: u32,
}

#[repr(C, packed)]
struct RsdpDescriptor20 {
    descriptor: RsdpDescriptor,
    length: u32,
    xsdt_address: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

#[repr(C)]
struct SdtHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

#[repr(C)]
struct MadtHeader {
    header: SdtHeader,
    local_apic_address: u32,
    flags: u32,
}

#[repr(C)]
struct MadtEntryHeader {
    entry_type: u8,
    length: u8,
}

pub fn discover(hhdm_offset: u64, rsdp_address: u64) -> Result<AcpiDiscovery, AcpiError> {
    let rsdp = phys_bytes(hhdm_offset, rsdp_address, size_of::<RsdpDescriptor>())?;
    if &rsdp[..RSDP_SIGNATURE.len()] != RSDP_SIGNATURE {
        return Err(AcpiError::InvalidRsdpSignature);
    }
    if checksum(rsdp) != 0 {
        return Err(AcpiError::InvalidRsdpChecksum);
    }

    let rsdp_ptr = phys_ptr::<RsdpDescriptor>(hhdm_offset, rsdp_address)?;
    let revision = unsafe { (*rsdp_ptr).revision };
    let oem_id = unsafe { (*rsdp_ptr).oem_id };

    let (root_kind, root_address) = if revision >= 2 {
        let rsdp20_ptr = phys_ptr::<RsdpDescriptor20>(hhdm_offset, rsdp_address)?;
        let length = unsafe { ptr::addr_of!((*rsdp20_ptr).length).read_unaligned() };
        if length < size_of::<RsdpDescriptor20>() as u32 {
            return Err(AcpiError::InvalidRsdpRevision);
        }

        let extended = phys_bytes(hhdm_offset, rsdp_address, length as usize)?;
        if checksum(extended) != 0 {
            return Err(AcpiError::InvalidRsdpChecksum);
        }

        let xsdt_address = unsafe { ptr::addr_of!((*rsdp20_ptr).xsdt_address).read_unaligned() };
        if xsdt_address != 0 {
            (RootKind::Xsdt, xsdt_address)
        } else {
            let rsdt_address = unsafe { (*rsdp_ptr).rsdt_address } as u64;
            if rsdt_address == 0 {
                return Err(AcpiError::MissingRootTable);
            }
            (RootKind::Rsdt, rsdt_address)
        }
    } else {
        let rsdt_address = unsafe { (*rsdp_ptr).rsdt_address } as u64;
        if rsdt_address == 0 {
            return Err(AcpiError::MissingRootTable);
        }
        (RootKind::Rsdt, rsdt_address)
    };

    let root_header_ptr = phys_ptr::<SdtHeader>(hhdm_offset, root_address)?;
    let root_header = unsafe { &*root_header_ptr };
    if root_header.length < size_of::<SdtHeader>() as u32 {
        return Err(AcpiError::InvalidRootLength);
    }

    let expected_signature = match root_kind {
        RootKind::Rsdt => SIGNATURE_RSDT,
        RootKind::Xsdt => SIGNATURE_XSDT,
    };
    if &root_header.signature != expected_signature {
        return Err(AcpiError::InvalidRootSignature);
    }

    let root_bytes = phys_bytes(hhdm_offset, root_address, root_header.length as usize)?;
    if checksum(root_bytes) != 0 {
        return Err(AcpiError::InvalidRootChecksum);
    }

    let entry_size = match root_kind {
        RootKind::Rsdt => size_of::<u32>(),
        RootKind::Xsdt => size_of::<u64>(),
    };
    let table_count = (root_bytes.len() - size_of::<SdtHeader>()) / entry_size;

    let mut valid_table_count = 0_usize;
    let mut invalid_table_count = 0_usize;
    let mut madt = None;
    let mut fadt = None;

    for index in 0..table_count {
        let entry_offset = size_of::<SdtHeader>() + (index * entry_size);
        let table_address = match root_kind {
            RootKind::Rsdt => read_u32(root_bytes, entry_offset) as u64,
            RootKind::Xsdt => read_u64(root_bytes, entry_offset),
        };

        if table_address == 0 {
            invalid_table_count += 1;
            continue;
        }

        let table_header_ptr = match phys_ptr::<SdtHeader>(hhdm_offset, table_address) {
            Ok(pointer) => pointer,
            Err(_) => {
                invalid_table_count += 1;
                continue;
            }
        };
        let table_header = unsafe { &*table_header_ptr };
        if table_header.length < size_of::<SdtHeader>() as u32 {
            invalid_table_count += 1;
            continue;
        }

        let table_bytes = match phys_bytes(hhdm_offset, table_address, table_header.length as usize) {
            Ok(bytes) => bytes,
            Err(_) => {
                invalid_table_count += 1;
                continue;
            }
        };
        if checksum(table_bytes) != 0 {
            invalid_table_count += 1;
            continue;
        }

        valid_table_count += 1;

        if madt.is_none() && table_header.signature == *SIGNATURE_MADT {
            madt = parse_madt(table_bytes);
        } else if fadt.is_none() && table_header.signature == *SIGNATURE_FADT {
            fadt = parse_fadt(table_header.revision, table_header.length, table_bytes);
        }
    }

    Ok(AcpiDiscovery {
        revision,
        oem_id,
        rsdp_address,
        root_address,
        root_kind,
        table_count,
        valid_table_count,
        invalid_table_count,
        madt,
        fadt,
    })
}

pub fn oem_id_str(oem_id: &[u8; 6]) -> &str {
    match str::from_utf8(oem_id) {
        Ok(value) => value.trim_end_matches(' '),
        Err(_) => "??????",
    }
}

fn parse_madt(table_bytes: &[u8]) -> Option<MadtInfo> {
    if table_bytes.len() < size_of::<MadtHeader>() {
        return None;
    }

    let header = unsafe { &*(table_bytes.as_ptr() as *const MadtHeader) };
    let mut local_apic_address = header.local_apic_address as u64;
    let mut processors = Vec::new();
    let mut io_apics = Vec::new();
    let mut interrupt_source_overrides = Vec::new();

    let mut offset = size_of::<MadtHeader>();
    while offset + size_of::<MadtEntryHeader>() <= table_bytes.len() {
        let entry_header = unsafe { &*(table_bytes.as_ptr().add(offset) as *const MadtEntryHeader) };
        let entry_length = entry_header.length as usize;
        if entry_length < size_of::<MadtEntryHeader>() || offset + entry_length > table_bytes.len() {
            break;
        }

        match entry_header.entry_type {
            MADT_ENTRY_LOCAL_APIC if entry_length >= 8 => {
                let processor_uid = read_u8(table_bytes, offset + 2) as u32;
                let apic_id = read_u8(table_bytes, offset + 3) as u32;
                let flags = read_u32(table_bytes, offset + 4);
                processors.push(MadtProcessor {
                    processor_uid,
                    apic_id,
                    enabled: flags & MADT_LOCAL_APIC_FLAG_ENABLED != 0,
                    online_capable: flags & MADT_LOCAL_APIC_FLAG_ONLINE_CAPABLE != 0,
                    x2apic: false,
                });
            }
            MADT_ENTRY_IO_APIC if entry_length >= 12 => {
                io_apics.push(IoApicInfo {
                    io_apic_id: read_u8(table_bytes, offset + 2),
                    address: read_u32(table_bytes, offset + 4),
                    global_system_interrupt_base: read_u32(table_bytes, offset + 8),
                });
            }
            MADT_ENTRY_INTERRUPT_SOURCE_OVERRIDE if entry_length >= 10 => {
                interrupt_source_overrides.push(InterruptSourceOverride {
                    source: read_u8(table_bytes, offset + 3),
                    global_system_interrupt: read_u32(table_bytes, offset + 4),
                    flags: read_u16(table_bytes, offset + 8),
                });
            }
            MADT_ENTRY_LOCAL_APIC_ADDRESS_OVERRIDE if entry_length >= 12 => {
                local_apic_address = read_u64(table_bytes, offset + 4);
            }
            MADT_ENTRY_LOCAL_X2APIC if entry_length >= 16 => {
                let apic_id = read_u32(table_bytes, offset + 4);
                let flags = read_u32(table_bytes, offset + 8);
                let processor_uid = read_u32(table_bytes, offset + 12);
                processors.push(MadtProcessor {
                    processor_uid,
                    apic_id,
                    enabled: flags & MADT_LOCAL_APIC_FLAG_ENABLED != 0,
                    online_capable: flags & MADT_LOCAL_APIC_FLAG_ONLINE_CAPABLE != 0,
                    x2apic: true,
                });
            }
            _ => {}
        }

        offset += entry_length;
    }

    Some(MadtInfo {
        local_apic_address,
        flags: header.flags,
        processors,
        io_apics,
        interrupt_source_overrides,
    })
}

fn parse_fadt(revision: u8, length: u32, table_bytes: &[u8]) -> Option<FadtInfo> {
    if table_bytes.len() < FADT_OFFSET_FLAGS + 4 {
        return None;
    }

    let flags = read_u32(table_bytes, FADT_OFFSET_FLAGS);
    let reset_register = if table_bytes.len() >= FADT_OFFSET_RESET_REGISTER + 12 {
        parse_generic_address(table_bytes, FADT_OFFSET_RESET_REGISTER)
    } else {
        None
    };

    Some(FadtInfo {
        revision,
        length,
        preferred_pm_profile: power_profile_from_raw(read_u8(table_bytes, FADT_OFFSET_PREFERRED_PM_PROFILE)),
        sci_interrupt: read_u16(table_bytes, FADT_OFFSET_SCI_INTERRUPT),
        smi_command_port: read_u32(table_bytes, FADT_OFFSET_SMI_COMMAND_PORT),
        acpi_enable: read_u8(table_bytes, FADT_OFFSET_ACPI_ENABLE),
        acpi_disable: read_u8(table_bytes, FADT_OFFSET_ACPI_DISABLE),
        pm1a_control_block: read_u32(table_bytes, FADT_OFFSET_PM1A_CONTROL_BLOCK),
        pm1b_control_block: read_u32(table_bytes, FADT_OFFSET_PM1B_CONTROL_BLOCK),
        boot_architecture_flags: if table_bytes.len() >= FADT_OFFSET_BOOT_ARCH_FLAGS + 2 {
            read_u16(table_bytes, FADT_OFFSET_BOOT_ARCH_FLAGS)
        } else {
            0
        },
        flags,
        reset_register,
        reset_value: if table_bytes.len() > FADT_OFFSET_RESET_VALUE {
            read_u8(table_bytes, FADT_OFFSET_RESET_VALUE)
        } else {
            0
        },
    })
}

fn parse_generic_address(table_bytes: &[u8], offset: usize) -> Option<GenericAddress> {
    if table_bytes.len() < offset + 12 {
        return None;
    }

    Some(GenericAddress {
        address_space: read_u8(table_bytes, offset),
        bit_width: read_u8(table_bytes, offset + 1),
        bit_offset: read_u8(table_bytes, offset + 2),
        access_size: read_u8(table_bytes, offset + 3),
        address: read_u64(table_bytes, offset + 4),
    })
}

fn power_profile_from_raw(raw: u8) -> PowerProfile {
    match raw {
        0 => PowerProfile::Unspecified,
        1 => PowerProfile::Desktop,
        2 => PowerProfile::Mobile,
        3 => PowerProfile::Workstation,
        4 => PowerProfile::EnterpriseServer,
        5 => PowerProfile::SohoServer,
        6 => PowerProfile::AppliancePc,
        7 => PowerProfile::PerformanceServer,
        8 => PowerProfile::Tablet,
        _ => PowerProfile::Unknown,
    }
}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0_u8, |sum, value| sum.wrapping_add(*value))
}

fn phys_ptr<T>(hhdm_offset: u64, physical_address: u64) -> Result<*const T, AcpiError> {
    let virtual_address = arch::x86_64::ensure_physical_region_mapped(
        hhdm_offset,
        physical_address,
        size_of::<T>(),
        0,
    )
    .map_err(map_error)?;
    Ok(virtual_address as *const T)
}

fn phys_bytes(
    hhdm_offset: u64,
    physical_address: u64,
    length: usize,
) -> Result<&'static [u8], AcpiError> {
    let virtual_address =
        arch::x86_64::ensure_physical_region_mapped(hhdm_offset, physical_address, length, 0)
            .map_err(map_error)?;
    Ok(unsafe { slice::from_raw_parts(virtual_address as *const u8, length) })
}

fn read_u8(bytes: &[u8], offset: usize) -> u8 {
    bytes[offset]
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    let mut raw = [0_u8; 2];
    raw.copy_from_slice(&bytes[offset..offset + 2]);
    u16::from_le_bytes(raw)
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    let mut raw = [0_u8; 4];
    raw.copy_from_slice(&bytes[offset..offset + 4]);
    u32::from_le_bytes(raw)
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    let mut raw = [0_u8; 8];
    raw.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(raw)
}

fn map_error(error: arch::x86_64::MapError) -> AcpiError {
    match error {
        arch::x86_64::MapError::AddressOverflow => AcpiError::AddressOverflow,
        arch::x86_64::MapError::PageTableAllocationFailed => AcpiError::MappingFailed,
    }
}
