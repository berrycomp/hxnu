#![feature(lang_items)]
// TCOL / HPL (HXNU Public License)
// This file is strictly governed by the HXNU Public License (HPL).
#![allow(static_mut_refs)]
#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(abi_x86_interrupt)]
#![feature(linkage)]

extern crate alloc;

mod acpi;
mod arch;
mod devfs;
pub mod drivers;
mod exec;
mod fat;
pub mod fb;
pub mod font;
mod init_exec;
mod initrd;
pub mod live_update;
pub mod secinter;
pub mod module_loader;
pub mod sxrc_core;
pub mod supernova;
pub mod hsched;

#[path = "../../../heterexec/src/lib.rs"]
pub mod hps;

pub mod hps_bridge;
pub mod neoio;
pub mod neoio_bridge;
#[macro_use]
mod log;
mod limine;
mod mm;
mod panic;
mod power;
mod runtime;
mod procfs;
mod sched;
mod serial;
mod smp;
mod syscall;
mod time;
mod tty;
mod uaccess;
mod vfs;

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::arch::asm;

const SELF_TEST: Option<SelfTest> = selected_self_test();

#[derive(Copy, Clone)]
enum SelfTest {
    Breakpoint,
    PageFault,
    GeneralProtectionFault,
    Panic,
    PowerReset,
    Heterexec,
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "C" fn _start() -> ! {
    serial::init();
    let clock_source = time::initialize();

    if !limine::base_revision_supported() {
        kprintln!("HXNU: unsupported Limine base revision");
        halt();
    }

    kprintln!("HXNU: x86_64 early bootstrap");
    kprintln!(
        "HXNU: log clock source = {}{}",
        clock_source.as_str(),
        if clock_source.is_estimated() { " (estimated)" } else { "" }
    );
    kprintln!("HXNU: Limine protocol handshake ok");

    let hhdm_offset = match limine::hhdm_offset() {
        Some(offset) => {
            kprintln!("HXNU: HHDM offset = {offset:#018x}");
            offset
        }
        None => {
            kprintln!("HXNU: HHDM response missing");
            halt();
        }
    };

    
    let tty = tty::initialize(false);
    kprintln!("HXNU: font engine online (UTF-8 rendering enabled: Latin, Türkçe [ç, ğ, ı, ö, ş, ü], Русский [а..я])");
    kprintln!("HXNU: framebuffer disabled for initrd minimalism, loading from /boot instead");
    kprintln!(
        "HXNU: tty console online id={} outputs={} framebuffer={} vcs={} geometry={}x{}",
        tty.console_id,
        tty.output_count,
        yes_no(tty.framebuffer_output),
        tty.virtual_console_count,
        tty.columns,
        tty.rows,
    );

    match limine::memory_map() {
        Some(memory_map) => {
            if memory_map.is_empty() {
                kprintln!("HXNU: memmap is empty");
                halt();
            }
            kprintln!("HXNU: memmap entries = {}", memory_map.len());
            if let Some(region) = memory_map.iter().find(|entry| entry.is_usable()) {
                kprintln!(
                    "HXNU: first usable region = base {:#018x}, size {} KiB",
                    region.base,
                    region.length / 1024
                );
            }

            match mm::frame::initialize(&memory_map) {
                Ok(stats) => {
                    kprintln!("HXNU: usable memory = {} KiB", stats.total_bytes / 1024);
                    kprintln!("HXNU: frame regions = {}", stats.usable_regions);
                    kprintln!("HXNU: allocatable frames = {}", stats.allocatable_bytes / mm::frame::PAGE_SIZE);
                    kprintln!(
                        "HXNU: allocatable memory = {} KiB",
                        stats.allocatable_bytes / 1024
                    );
                    match mm::frame::allocate_frame() {
                        Some(frame) => kprintln!(
                            "HXNU: bootstrap frame = {:#018x}",
                            frame.start_address()
                        ),
                        None => {
                            kprintln!("HXNU: frame allocator returned no frame");
                            halt();
                        }
                    }
                    let stats = mm::frame::stats();
                    kprintln!("HXNU: allocated frames = {}", stats.allocated_frames);
                }
                Err(error) => {
                    kprintln!("HXNU: frame allocator init failed: {:?}", error);
                    halt();
                }
            }
        }
        None => kprintln!("HXNU: memmap response missing"),
    }

    match mm::heap::initialize(hhdm_offset) {
        Ok(stats) => {
            kprintln!("HXNU: heap start = {:#018x}", stats.start);
            kprintln!("HXNU: heap size = {} KiB", stats.size_bytes / 1024);

            let boxed = Box::new(0x4858_4e55_u32);
            kprintln!("HXNU: boxed value = {:#010x}", *boxed);

            let mut values = Vec::new();
            values.push(3_u64);
            values.push(1_u64);
            values.push(4_u64);
            values.push(1_u64);
            values.push(5_u64);
            let sum: u64 = values.iter().copied().sum();
            kprintln!("HXNU: vec len = {}, sum = {}", values.len(), sum);

            let stats = mm::heap::stats();
            kprintln!("HXNU: heap used = {} bytes", stats.used_bytes);
            kprintln!("HXNU: heap allocations = {}", stats.allocation_count);
        }
        Err(error) => {
            kprintln!("HXNU: heap init failed: {:?}", error);
            halt();
        }
    }

    match arch::x86_64::ensure_physical_region_mapped(
        hhdm_offset,
        0xFE00_0000,
        0x2000,
        arch::x86_64::FLAG_WRITE_THROUGH
            | arch::x86_64::FLAG_CACHE_DISABLE
            | arch::x86_64::FLAG_GLOBAL
            | arch::x86_64::FLAG_WRITABLE,
    ) {
        Ok(virt_addr) => kprintln!("HXNU: Supernova MMIO mapped globally into HHDM at {virt_addr:#018x}"),
        Err(_) => kprintln!("HXNU: Supernova MMIO global mapping failed"),
    }
    let supernova_driver = supernova::SupernovaDriver::new();
    supernova_driver.init();

    arch::x86_64::initialize();
    let selectors = arch::x86_64::segment_selectors();
    kprintln!(
        "HXNU: x86_64 descriptor tables loaded cs={:#06x} ds={:#06x} es={:#06x} fs={:#06x} gs={:#06x} ss={:#06x}",
        selectors.cs,
        selectors.ds,
        selectors.es,
        selectors.fs,
        selectors.gs,
        selectors.ss,
    );
    let cpu_info = arch::x86_64::probe_cpu();
    kprintln!(
        "HXNU: cpu local-apic={} x2apic={} tsc-deadline={} invariant-tsc={} nx={} hypervisor={} initial-apic-id={}",
        yes_no(cpu_info.local_apic_supported),
        yes_no(cpu_info.x2apic_supported),
        yes_no(cpu_info.tsc_deadline_supported),
        yes_no(cpu_info.invariant_tsc_supported),
        yes_no(cpu_info.nx_supported),
        yes_no(cpu_info.hypervisor_present),
        cpu_info.initial_apic_id,
    );
    kprintln_style!(
        crate::tty::ConsoleStyle::Muted,
        "HXNU: cpuid vendor={} vendor-id={} max-basic={:#x} max-extended={:#x}",
        cpu_info.vendor.as_str(),
        cpu_info.vendor_str(),
        cpu_info.max_basic_leaf,
        cpu_info.max_extended_leaf,
    );
    if let Some(brand) = cpu_info.brand_str() {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: cpuid brand {}",
            brand,
        );
    }
    if let Some(topology) = cpu_info.topology {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: cpuid topology leaf={} x2apic-id={} smt-shift={} core-shift={} threads/core={} logical/package={} smt-id={} core-id={} package-id={}",
            topology.leaf_kind.as_str(),
            topology.x2apic_id,
            topology.smt_shift,
            topology.core_shift,
            topology.threads_per_core,
            topology.logical_processors_per_package,
            topology.smt_id,
            topology.core_id,
            topology.package_id,
        );
        for level in topology.levels[..topology.level_count].iter() {
            kprintln_style!(
                crate::tty::ConsoleStyle::Muted,
                "HXNU: cpuid topo level={} type={} shift={} logical={} x2apic-id={}",
                level.level_number,
                level.level_type.as_str(),
                level.shift,
                level.logical_processors,
                level.x2apic_id,
            );
        }
    } else {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: cpuid topology leaf unavailable"
        );
    }
    if cpu_info.local_apic_supported {
        kprintln!(
            "HXNU: apic base={:#010x} enabled={} x2apic-mode={} bsp={}",
            cpu_info.apic_base,
            yes_no(cpu_info.apic_global_enabled),
            yes_no(cpu_info.x2apic_enabled),
            yes_no(cpu_info.bootstrap_processor),
        );
    }
    match arch::x86_64::initialize_local_apic_timer(hhdm_offset, &cpu_info) {
        Ok(timer) => kprintln!(
            "HXNU: apic timer online vector={:#04x} divide={} initial-count={} ticks={}",
            timer.vector,
            timer.divide_value,
            timer.initial_count,
            timer.ticks_observed,
        ),
        Err(error) => kprintln!("HXNU: apic timer offline reason={}", error.as_str()),
    }

    match limine::rsdp_address() {
        Some(rsdp_address) => {
            kprintln!("HXNU: acpi rsdp response @ {:#010x}", rsdp_address);
            match acpi::discover(hhdm_offset, rsdp_address) {
                Ok(discovery) => {
                    kprintln_style!(
                        crate::tty::ConsoleStyle::Accent,
                        "HXNU: acpi online revision={} oem={} rsdp={:#010x} root={} @ {:#010x}",
                        discovery.revision,
                        acpi::oem_id_str(&discovery.oem_id),
                        discovery.rsdp_address,
                        discovery.root_kind.as_str(),
                        discovery.root_address,
                    );
                    kprintln!(
                        "HXNU: acpi tables total={} valid={} invalid={} madt={} fadt={}",
                        discovery.table_count,
                        discovery.valid_table_count,
                        discovery.invalid_table_count,
                        yes_no(discovery.madt.is_some()),
                        yes_no(discovery.fadt.is_some()),
                    );
                    if let Some(ref madt) = discovery.madt {
                        kprintln_style!(
                            crate::tty::ConsoleStyle::Accent,
                            "HXNU: acpi madt lapic={:#010x} flags={:#010x} cpus-enabled={}/{} ioapics={} iso={} x2apic-cpus={}",
                            madt.local_apic_address,
                            madt.flags,
                            madt.enabled_processor_count(),
                            madt.total_processor_count(),
                            madt.io_apics.len(),
                            madt.interrupt_source_overrides.len(),
                            madt.local_x2apic_count(),
                        );
                        if let Some(processor) = madt.processors.first() {
                            kprintln!(
                                "HXNU: acpi cpu0 uid={} apic={} mode={} enabled={} online-capable={}",
                                processor.processor_uid,
                                processor.apic_id,
                                processor.apic_mode(),
                                yes_no(processor.enabled),
                                yes_no(processor.online_capable),
                            );
                        }
                        if let Some(io_apic) = madt.io_apics.first() {
                            kprintln!(
                                "HXNU: acpi ioapic0 id={} addr={:#010x} gsi-base={}",
                                io_apic.io_apic_id,
                                io_apic.address,
                                io_apic.global_system_interrupt_base,
                            );
                        }
                        if let Some(override_entry) = madt.interrupt_source_overrides.first() {
                            kprintln!(
                                "HXNU: acpi iso0 source={} gsi={} flags={:#06x}",
                                override_entry.source,
                                override_entry.global_system_interrupt,
                                override_entry.flags,
                            );
                        }
                        match smp::initialize(&cpu_info, madt) {
                            Ok(summary) => {
                                kprintln_style!(
                                    crate::tty::ConsoleStyle::Success,
                                    "HXNU: smp topology bsp-apic={} bsp-index={} cpus={} enabled={} online={} aps={} bringup-targets={} x2apic={}",
                                    summary.bsp_apic_id,
                                    summary.current_cpu_index,
                                    summary.total_cpus,
                                    summary.enabled_cpus,
                                    summary.online_cpus,
                                    summary.ap_count,
                                    summary.bringup_targets,
                                    summary.x2apic_cpus,
                                );
                                if let Some(topology) = smp::topology() {
                                    let current = topology.current_cpu();
                                    kprintln!(
                                        "HXNU: smp current cpu{} uid={} apic={} mode={} bsp={} online={}",
                                        current.index,
                                        current.processor_uid,
                                        current.apic_id,
                                        current.apic_mode(),
                                        yes_no(current.is_bsp),
                                        yes_no(current.online),
                                    );
                                    if let Some(target) = topology.first_bringup_target() {
                                        kprintln_style!(
                                            crate::tty::ConsoleStyle::Muted,
                                            "HXNU: smp next ap target cpu{} uid={} apic={} mode={} online-capable={}",
                                            target.index,
                                            target.processor_uid,
                                            target.apic_id,
                                            target.apic_mode(),
                                            yes_no(target.online_capable),
                                        );
                                    }
                                }
                            }
                            Err(error) => kprintln_style!(
                                crate::tty::ConsoleStyle::Error,
                                "HXNU: smp topology offline reason={}",
                                error.as_str()
                            ),
                        }
                    }
                    if let Some(ref fadt) = discovery.fadt {
                        power::configure(hhdm_offset, fadt);
                        kprintln!(
                            "HXNU: acpi fadt revision={} length={} profile={} sci={} smi-cmd={:#x}",
                            fadt.revision,
                            fadt.length,
                            fadt.preferred_pm_profile.as_str(),
                            fadt.sci_interrupt,
                            fadt.smi_command_port,
                        );
                        kprintln_style!(
                            crate::tty::ConsoleStyle::Warning,
                            "HXNU: acpi power reset={} hw-reduced={} pm1a-ctl={:#x} pm1b-ctl={:#x}",
                            yes_no(fadt.reset_supported()),
                            yes_no(fadt.hardware_reduced()),
                            fadt.pm1a_control_block,
                            fadt.pm1b_control_block,
                        );
                        if let Some(reset_register) = fadt.reset_register {
                            kprintln!(
                                "HXNU: acpi reset-reg space={} width={} offset={} access={} addr={:#x} value={:#04x}",
                                reset_register.address_space_str(),
                                reset_register.bit_width,
                                reset_register.bit_offset,
                                reset_register.access_size,
                                reset_register.address,
                                fadt.reset_value,
                            );
                        }
                        kprintln!(
                            "HXNU: acpi boot-arch flags={:#06x} acpi-enable={:#04x} acpi-disable={:#04x}",
                            fadt.boot_architecture_flags,
                            fadt.acpi_enable,
                            fadt.acpi_disable,
                        );
                    }
                }
                Err(error) => kprintln!("HXNU: acpi offline reason={}", error.as_str()),
            }
        }
        None => kprintln!("HXNU: acpi rsdp response missing"),
    }

    match procfs::initialize(&cpu_info) {
        Ok(summary) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: procfs online directories={} files={} entries={}",
            summary.directory_count,
            summary.file_count,
            summary.entry_count,
        ),
        Err(error) => {
            kprintln_style!(
                crate::tty::ConsoleStyle::Error,
                "HXNU: procfs offline reason={}",
                error.as_str()
            );
            halt();
        }
    }
    match devfs::initialize() {
        Ok(summary) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: devfs online directories={} nodes={} entries={}",
            summary.directory_count,
            summary.node_count,
            summary.entry_count,
        ),
        Err(error) => {
            kprintln_style!(
                crate::tty::ConsoleStyle::Error,
                "HXNU: devfs offline reason={}",
                error.as_str()
            );
            halt();
        }
    }
    match initrd::initialize() {
        Ok(summary) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: initrd online modules={} directories={} files={} entries={} bytes={} path={} label={}",
            summary.module_count,
            summary.directory_count,
            summary.file_count,
            summary.entry_count,
            summary.archive_bytes,
            initrd::module_path().unwrap_or("<unknown>"),
            initrd::module_label().unwrap_or("<none>"),
        ),
        Err(error) => kprintln_style!(
            crate::tty::ConsoleStyle::Warning,
            "HXNU: initrd offline reason={}",
            error.as_str()
        ),
    }
    match fat::initialize() {
        Ok(summary) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: fat online mounted={} partition={} device={} table={} type={} root-entries={} directories={}",
            yes_no(summary.mounted),
            summary
                .partition_id
                .map_or(String::from("<none>"), |value| value.to_string()),
            summary
                .device_id
                .map_or(String::from("<none>"), |value| value.to_string()),
            summary
                .partition_table
                .map_or("<none>", |kind| kind.as_str()),
            summary.fat_type.map_or("<none>", |kind| kind.as_str()),
            summary.root_entry_count,
            summary.directory_count,
        ),
        Err(error) => kprintln_style!(
            crate::tty::ConsoleStyle::Warning,
            "HXNU: fat offline reason={}",
            error.as_str()
        ),
    }
    match vfs::initialize() {
        Ok(summary) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: vfs online mounts={} root-entries={} directories={}",
            summary.mount_count,
            summary.root_entry_count,
            summary.directory_count,
        ),
        Err(error) => {
            kprintln_style!(
                crate::tty::ConsoleStyle::Error,
                "HXNU: vfs offline reason={}",
                error.as_str()
            );
            halt();
        }
    }
    let hfs_summary = initialize_hfs();
    kprintln_style!(
        crate::tty::ConsoleStyle::Success,
        "HXNU: hfs online mounted=yes path=/hfs namespace=128-bit Lut entries={} shadowed={} hps-halted={}",
        hfs_summary.0,
        hfs_summary.1,
        yes_no(hfs_summary.2),
    );
    module_loader::load_modules();
    match vfs::discover_init_executable() {
        Ok(candidate) => kprintln_style!(
            crate::tty::ConsoleStyle::Accent,
            "HXNU: init candidate path={} mount={} format={} size={} executable={}",
            candidate.path,
            candidate.mount.as_str(),
            candidate.format.as_str(),
            candidate.size,
            yes_no(candidate.executable),
        ),
        Err(error) => kprintln_style!(
            crate::tty::ConsoleStyle::Warning,
            "HXNU: init candidate offline reason={}",
            error.as_str()
        ),
    }
    let init_load_prep = vfs::prepare_init_load();
    match &init_load_prep {
        Ok(prep) => kprintln_style!(
            crate::tty::ConsoleStyle::Accent,
            "HXNU: init load-prep path={} mount={} format={} size={} executable={} entry={} machine={} type={} ph={} load={} load-base={} load-offset={} load-file={} load-mem={} load-w={} load-x={} align={} vm-map={} vm-bytes={} vm-zero={} vm-start={} vm-end={} interp={} interp-src={} interp-ok={} interp-arg={}",
            prep.path,
            prep.mount.as_str(),
            prep.format.as_str(),
            prep.size,
            yes_no(prep.executable),
            vfs::format_u64_hex(prep.entry_point),
            vfs::format_u16_hex(prep.machine),
            vfs::format_u16_hex(prep.image_type),
            prep.program_header_count,
            prep.load_segment_count,
            vfs::format_u64_hex(prep.load_base),
            vfs::format_u64_hex(prep.load_offset),
            prep.load_file_bytes,
            prep.load_memory_bytes,
            prep.writable_load_segments,
            prep.executable_load_segments,
            prep.max_alignment,
            prep.vm_map_entries.len(),
            prep.vm_map_total_bytes,
            prep.vm_map_zero_fill_bytes,
            vfs::format_u64_hex(prep.vm_map_start),
            vfs::format_u64_hex(prep.vm_map_end),
            prep.interpreter.as_deref().unwrap_or("<none>"),
            prep.interpreter_source.as_deref().unwrap_or("<none>"),
            yes_no(prep.interpreter_resolved),
            prep.interpreter_argument.as_deref().unwrap_or("<none>"),
        ),
        Err(error) => kprintln_style!(
            crate::tty::ConsoleStyle::Warning,
            "HXNU: init load-prep offline reason={}",
            error.as_str()
        ),
    }
    if let Ok(prep) = &init_load_prep {
        if let Some(segment) = prep.vm_map_entries.first() {
            kprintln_style!(
                crate::tty::ConsoleStyle::Muted,
                "HXNU: init vm-map[0] idx={} off={} vaddr={}..{} map={}..{} page-off={} file={} mem={} zero={} align={} perms={}",
                segment.index,
                vfs::format_u64_hex(Some(segment.file_offset)),
                vfs::format_u64_hex(Some(segment.virtual_start)),
                vfs::format_u64_hex(Some(segment.virtual_end)),
                vfs::format_u64_hex(Some(segment.map_start)),
                vfs::format_u64_hex(Some(segment.map_end)),
                segment.page_offset,
                segment.file_bytes,
                segment.memory_bytes,
                segment.zero_fill_bytes,
                segment.alignment,
                vfs::format_rwx(segment.readable, segment.writable, segment.executable),
            );
        }
    }
    for console_id in 1..tty::VIRTUAL_CONSOLE_COUNT as u32 {
        let _ = tty::write_to_console(
            console_id,
            crate::tty::ConsoleStyle::Accent,
            "HXNU virtual console ready\n",
        );
        let _ = tty::write_to_console(
            console_id,
            crate::tty::ConsoleStyle::Muted,
            "Framebuffer redraw path prepared for multi-screen TTY\n",
        );
    }
    match tty::switch_active_console(1) {
        Ok(()) => {
            let _ = tty::switch_active_console(0);
            let tty = tty::stats();
            kprintln_style!(
                crate::tty::ConsoleStyle::Success,
                "HXNU: tty virtual consoles online active=tty{} total={} geometry={}x{} switch-smoke=yes",
                tty.console_id,
                tty.virtual_console_count,
                tty.columns,
                tty.rows,
            );
        }
        Err(error) => {
            kprintln_style!(
                crate::tty::ConsoleStyle::Error,
                "HXNU: tty virtual console switch failed reason={}",
                error.as_str()
            );
            halt();
        }
    }

    if let Some(test) = SELF_TEST {
        match test {
            SelfTest::Breakpoint => {
                kprintln!("HXNU: running exception self-test = breakpoint");
                arch::x86_64::run_exception_self_test(arch::x86_64::ExceptionSelfTest::Breakpoint);
                kprintln!("HXNU: breakpoint handler returned");
            }
            SelfTest::PageFault => {
                kprintln!("HXNU: running exception self-test = page-fault");
                arch::x86_64::run_exception_self_test(arch::x86_64::ExceptionSelfTest::PageFault);
                kprintln!("HXNU: page-fault self-test unexpectedly returned");
                halt();
            }
            SelfTest::GeneralProtectionFault => {
                kprintln!("HXNU: running exception self-test = general-protection-fault");
                arch::x86_64::run_exception_self_test(
                    arch::x86_64::ExceptionSelfTest::GeneralProtectionFault,
                );
                kprintln!("HXNU: gpf self-test unexpectedly returned");
                halt();
            }
            SelfTest::Panic => {
                kprintln!("HXNU: running kernel self-test = panic");
                panic!("requested kernel panic self-test");
            }
            SelfTest::Heterexec => {
                kprintln!("HXNU: running kernel self-test = heterexec bridge");
                unsafe {
                    crate::hsched::HSCHED.init();
                    let ptr = crate::hps_bridge::hps_get_shared_buffer();
                    kprintln!("HXNU: heterexec hook shared_buffer ptr={:p}", ptr);
                    
                    let id1 = crate::hsched::HSCHED.submit_workload(crate::hsched::WorkloadType::CpuAvx512).unwrap();
                    let id2 = crate::hsched::HSCHED.submit_workload(crate::hsched::WorkloadType::GpuCompute).unwrap();
                    
                    kprintln!("HXNU: submitted workloads id1={} id2={}", id1, id2);
                    
                    let tail = crate::hsched::HSCHED.shared_buffer.tail.load(core::sync::atomic::Ordering::SeqCst);
                    let head = crate::hsched::HSCHED.shared_buffer.head.load(core::sync::atomic::Ordering::SeqCst);
                    assert_eq!(tail.wrapping_sub(head), 2, "HXNU: Expected 2 pending workloads");
                    
                    crate::hsched::HSCHED.dispatch_pending();
                    
                    let tail2 = crate::hsched::HSCHED.shared_buffer.tail.load(core::sync::atomic::Ordering::SeqCst);
                    let head2 = crate::hsched::HSCHED.shared_buffer.head.load(core::sync::atomic::Ordering::SeqCst);
                    assert_eq!(tail2, head2, "HXNU: Expected all workloads to be dispatched");
                    
                    kprintln!("HXNU: dispatched workloads");

                    let test_addr = crate::hps::hfs::HfsAddress([0x01; 16]);
                    if let Some(session) = crate::hps::loader::HxeSession::load_from_hfs(test_addr) {
                        kprintln!(
                            "HXNU: HxeSession::load_from_hfs PASSED storage_tier={:?} uma_shared_size={}",
                            session.storage_tier,
                            session.uma_shared_size
                        );
                    } else {
                        kprintln!("HXNU: HxeSession::load_from_hfs FAILED");
                    }

                    kprintln!("HXNU: Heterexec bridge self-test PASSED");
                }
            }
            SelfTest::PowerReset => {
                let capability = power::reset_capability();
                kprintln!(
                    "HXNU: running kernel self-test = power-reset supported={} space={} addr={:#x} value={:#04x}",
                    yes_no(capability.supported),
                    capability.address_space_str(),
                    capability.address,
                    capability.value,
                );
                match power::reboot() {
                    Ok(()) => power::halt_forever(),
                    Err(error) => {
                        kprintln_style!(
                            crate::tty::ConsoleStyle::Error,
                            "HXNU: power reset self-test failed reason={}",
                            error.as_str()
                        );
                        halt();
                    }
                }
            }
        }
    }

    if crate::drivers::bga::is_available() {
        kprintln!("HXNU: display hardware detected: Bochs BGA");
        let caps = crate::drivers::bga::probe_caps();
        let target_w = 1920.min(caps.max_width as u32);
        let target_h = 1080.min(caps.max_height as u32);
        let target_bpp = 32.min(caps.max_bpp);

        let mode = crate::drivers::bga::set_mode(target_w, target_h, target_bpp);
        kprintln!(
            "HXNU: dynamic resolution updated: {}x{}@{}bpp (maximum supported limit)",
            mode.width,
            mode.height,
            mode.bpp
        );

        if let Some(boot_fb) = limine::framebuffer() {
            let phys_addr = (boot_fb.address as u64) - hhdm_offset;
            let fb_size = (mode.pitch as usize) * (mode.height as usize);

            let virt_addr = match arch::x86_64::ensure_region_mapped(
                hhdm_offset,
                phys_addr,
                fb_size,
                arch::x86_64::FLAG_WRITABLE | arch::x86_64::FLAG_WRITE_THROUGH,
            ) {
                Ok(vaddr) => vaddr,
                Err(_) => boot_fb.address as u64,
            };

            fb::set_physical_address(phys_addr);

            let updated_fb = limine::Framebuffer {
                address: virt_addr as *mut u8,
                width: mode.width as u64,
                height: mode.height as u64,
                pitch: mode.pitch as u64,
                bpp: mode.bpp,
                memory_model: boot_fb.memory_model,
                red_mask_size: if boot_fb.red_mask_size == 0 { 8 } else { boot_fb.red_mask_size },
                red_mask_shift: if boot_fb.red_mask_size == 0 { 16 } else { boot_fb.red_mask_shift },
                green_mask_size: if boot_fb.green_mask_size == 0 { 8 } else { boot_fb.green_mask_size },
                green_mask_shift: if boot_fb.green_mask_size == 0 { 8 } else { boot_fb.green_mask_shift },
                blue_mask_size: if boot_fb.blue_mask_size == 0 { 8 } else { boot_fb.blue_mask_size },
                blue_mask_shift: if boot_fb.blue_mask_shift == 0 { 0 } else { boot_fb.blue_mask_shift },
            };

            if fb::initialize(updated_fb).is_ok() {
                tty::initialize(true);
                kprintln!("HXNU: framebuffer initialized for userspace handoff");
            }
        }
    }

    let init_activation = init_exec::activate_init_handoff();
    match &init_activation {
        Ok(summary) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: init activation armed={} entry={:#018x} vm={:#018x}..{:#018x} segments={} bytes={} zero={}",
            yes_no(summary.armed),
            summary.entry_point,
            summary.vm_start,
            summary.vm_end,
            summary.segment_count,
            summary.total_bytes,
            summary.zero_fill_bytes,
        ),
        Err(error) => kprintln_style!(
            crate::tty::ConsoleStyle::Error,
            "HXNU: init activation failed reason={}",
            error.as_str(),
        ),
    }

    match sched::bootstrap(hhdm_offset, &cpu_info) {
        Ok(state) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: scheduler bootstrap online source={} vector={:#04x} divide={} initial-count={} ticks={} threads={} runqueue={} current={}#{} pid={} ppid={} role={} switches={} bootstrap-id={} idle-id={}",
            state.source,
            state.vector,
            state.divide_value,
            state.initial_count,
            state.ticks_observed,
            state.thread_count,
            state.runqueue_depth,
            state.current_thread_name,
            state.current_thread_id,
            state.current_process_id,
            state.current_parent_process_id,
            state.current_thread_role,
            state.context_switches,
            state.bootstrap_thread_id,
            state.idle_thread_id,
        ),
        Err(error) => {
            kprintln!("HXNU: scheduler bootstrap offline reason={}", error.as_str());
            halt();
        }
    }

    match init_exec::spawn_init_process() {
        Ok(spawned) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: init process spawned pid={} thread_id={} restart-count={}",
            spawned.process_id,
            spawned.thread_id,
            spawned.restart_count,
        ),
        Err(error) => kprintln_style!(
            crate::tty::ConsoleStyle::Error,
            "HXNU: init process spawn failed reason={}",
            error.as_str()
        ),
    }

    let tty_stats = tty::stats();
    let scheduler_stats = sched::stats();
    kprintln!(
        "HXNU: tty stats id={} outputs={} bytes={} lines={}",
        tty_stats.console_id,
        tty_stats.output_count,
        tty_stats.bytes_written,
        tty_stats.lines_written,
    );
    kprintln!(
        "HXNU: scheduler stats threads={} runqueue={} current={}#{} pid={} ppid={} state={} ticks={} switches={} bootstrap-id={} idle-id={}",
        scheduler_stats.thread_count,
        scheduler_stats.runqueue_depth,
        scheduler_stats.current_thread_name,
        scheduler_stats.current_thread_id,
        scheduler_stats.current_process_id,
        scheduler_stats.current_parent_process_id,
        scheduler_stats.current_thread_state,
        scheduler_stats.total_ticks,
        scheduler_stats.context_switches,
        scheduler_stats.bootstrap_thread_id,
        scheduler_stats.idle_thread_id,
    );
    let linux_probe = syscall::run_linux_bootstrap_probe();
    kprintln_style!(
        crate::tty::ConsoleStyle::Success,
        "HXNU: syscall bootstrap abi={} write={} openat={} read={} close={} getpid={} getppid={} gettid={} sched_yield={} clock_gettime={} monotonic={}.{:09} uname={} machine={} exit-captured={} exit-status={}",
        syscall::SyscallAbi::LinuxBootstrap.as_str(),
        linux_probe.write_result,
        linux_probe.openat_result,
        linux_probe.read_result,
        linux_probe.close_result,
        linux_probe.getpid_result,
        linux_probe.getppid_result,
        linux_probe.gettid_result,
        linux_probe.sched_yield_result,
        linux_probe.clock_gettime_result,
        linux_probe.clock_seconds,
        linux_probe.clock_nanoseconds,
        linux_probe.uname_result,
        linux_probe.machine_str(),
        yes_no(linux_probe.exit_group_captured),
        linux_probe.exit_group_status,
    );
    let ghost_probe = syscall::run_ghost_bootstrap_probe();
    kprintln_style!(
        crate::tty::ConsoleStyle::Success,
        "HXNU: syscall bootstrap abi={} write={} open={} read={} close={} getpid={} gettid={} yield={} uptime-ns={} uname={} machine={} exit-captured={} exit-status={}",
        syscall::SyscallAbi::GhostBootstrap.as_str(),
        ghost_probe.write_result,
        ghost_probe.open_result,
        ghost_probe.read_result,
        ghost_probe.close_result,
        ghost_probe.getpid_result,
        ghost_probe.gettid_result,
        ghost_probe.yield_result,
        ghost_probe.uptime_result,
        ghost_probe.uname_result,
        ghost_probe.machine_str(),
        yes_no(ghost_probe.exit_group_captured),
        ghost_probe.exit_group_status,
    );
    let hxnu_probe = syscall::run_hxnu_bootstrap_probe();
    kprintln_style!(
        crate::tty::ConsoleStyle::Success,
        "HXNU: syscall bootstrap abi={} log_write={} open={} read={} close={} process_self={} thread_self={} sched_yield={} uptime-ns={} abi-version={:#x} exit-captured={} exit-status={}",
        syscall::SyscallAbi::HxnuNativeBootstrap.as_str(),
        hxnu_probe.write_result,
        hxnu_probe.open_result,
        hxnu_probe.read_result,
        hxnu_probe.close_result,
        hxnu_probe.process_self_result,
        hxnu_probe.thread_self_result,
        hxnu_probe.sched_yield_result,
        hxnu_probe.uptime_result,
        hxnu_probe.abi_version_result,
        yes_no(hxnu_probe.exit_group_captured),
        hxnu_probe.exit_group_status,
    );
    let syscall_self_test = arch::x86_64::run_syscall_self_test();
    kprintln_style!(
        crate::tty::ConsoleStyle::Success,
        "HXNU: syscall entry self-test int=0x80 linux_write={} linux_openat={} linux_read={} linux_close={} linux_getpid={} ghost_open={} ghost_read={} ghost_close={} ghost_gettid={} hxnu_open={} hxnu_read={} hxnu_close={} hxnu_abi_version={:#x}",
        syscall_self_test.linux_write_result,
        syscall_self_test.linux_openat_result,
        syscall_self_test.linux_read_result,
        syscall_self_test.linux_close_result,
        syscall_self_test.linux_getpid_result,
        syscall_self_test.ghost_open_result,
        syscall_self_test.ghost_read_result,
        syscall_self_test.ghost_close_result,
        syscall_self_test.ghost_gettid_result,
        syscall_self_test.hxnu_open_result,
        syscall_self_test.hxnu_read_result,
        syscall_self_test.hxnu_close_result,
        syscall_self_test.hxnu_abi_version_result,
    );
    if let Some(root) = vfs::preview("/", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: vfs preview root={}",
            root,
        );
    }
    if let Some(version) = vfs::preview("/proc/version", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: procfs preview version={}",
            version,
        );
    }
    if let Some(uptime) = vfs::preview("/proc/uptime", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: procfs preview uptime={}",
            uptime,
        );
    }
    if let Some(schedstat) = vfs::preview("/proc/schedstat", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: procfs preview schedstat={}",
            schedstat,
        );
    }
    if let Some(devlist) = vfs::preview("/dev", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: devfs preview root={}",
            devlist,
        );
    }
    if let Some(initrd_root) = vfs::preview("/initrd", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: initrd preview root={}",
            initrd_root,
        );
    }
    if let Some(fat_root) = vfs::preview("/boot", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: fat preview root={}",
            fat_root,
        );
    }
    if let Some(console) = vfs::preview("/dev/console", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: devfs preview console={}",
            console,
        );
    }
    if let Some(fb0) = vfs::preview("/dev/fb0", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: devfs preview fb0={}",
            fb0,
        );
    }

    kprintln!("HXNU: Rust kernel skeleton online");
    
    unsafe {
        hps::init_hxext(
            core::mem::transmute(crate::hps_bridge::hps_get_shared_buffer as *const ()),
            core::mem::transmute(crate::neoio_bridge::hps_get_topology_tree as *const ()),
        );
    }

    crate::sched::register_hps_bridge(test_hps_hook);
    crate::sched::trigger_hps_bridge(42, true);
    let val = HPS_HOOK_CALLED.load(core::sync::atomic::Ordering::SeqCst);
    kprintln!("HXNU: HPS_HOOK_CALLED={}", val);
    
    sched::idle_loop()
}

const fn selected_self_test() -> Option<SelfTest> {
    if cfg!(feature = "panic-self-test") {
        Some(SelfTest::Panic)
    } else if cfg!(feature = "power-reset-self-test") {
        Some(SelfTest::PowerReset)
    } else if cfg!(feature = "heterexec-self-test") {
        Some(SelfTest::Heterexec)
    } else if cfg!(feature = "exception-test-page-fault") {
        Some(SelfTest::PageFault)
    } else if cfg!(feature = "exception-test-general-protection") {
        Some(SelfTest::GeneralProtectionFault)
    } else if cfg!(feature = "exception-test-breakpoint") {
        Some(SelfTest::Breakpoint)
    } else {
        Some(SelfTest::Breakpoint)
    }
}

fn halt() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

use core::sync::atomic::{AtomicUsize, Ordering};
pub static HPS_HOOK_CALLED: AtomicUsize = AtomicUsize::new(0);

pub fn test_hps_hook(thread_id: u64, is_gpu: bool) {
    HPS_HOOK_CALLED.fetch_add(1, Ordering::SeqCst);
    crate::serial::write_str("HXNU: stress_test_hps_hook triggered!\n");
}

static mut HXE_TEST_PAYLOAD: [u8; 64] = [0u8; 64];

fn initialize_hfs() -> (usize, usize, bool) {
    use crate::hps::hfs::{HfsAddress, PowerLossPrevention, StorageTier, UnifiedNamespace};

    // Restore state if booting after outage event
    let restored = PowerLossPrevention::recover_from_emergency_dump();
    if restored > 0 {
        kprintln!(
            "HXNU: hfs outage recovery restored {} entries from NVMe hibernate block",
            restored
        );
    }

    // Register test HXE payload for heterexec loader compatibility
    unsafe {
        let magic = b"\x7FHXE";
        HXE_TEST_PAYLOAD[0..4].copy_from_slice(magic);
        HXE_TEST_PAYLOAD[4..6].copy_from_slice(&1u16.to_le_bytes()); // Version
        HXE_TEST_PAYLOAD[6..10].copy_from_slice(&0u32.to_le_bytes()); // HardwareMask
        HXE_TEST_PAYLOAD[10..18].copy_from_slice(&4096u64.to_le_bytes()); // UMA shared size
    }

    let test_addr = HfsAddress([0x01; 16]);
    UnifiedNamespace::register(
        test_addr,
        StorageTier::VRAM,
        unsafe { HXE_TEST_PAYLOAD.as_mut_ptr() as *mut core::ffi::c_void },
        64,
    );

    // Dynamic tier shift
    UnifiedNamespace::shift_tier(test_addr, StorageTier::VRAM);

    // continuous dma shadowing
    unsafe {
        PowerLossPrevention::continuous_dma_shadowing(test_addr);
    }

    (
        UnifiedNamespace::active_count(),
        PowerLossPrevention::shadow_count(),
        PowerLossPrevention::is_hps_halted(),
    )
}

#[lang = "eh_personality"]
fn eh_personality() {}
