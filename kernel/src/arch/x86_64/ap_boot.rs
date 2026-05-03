use core::arch::{asm, x86_64::_rdtsc};
use core::sync::atomic::{AtomicU32, Ordering};

use crate::arch;
use crate::kprintln;
use crate::mm;
use crate::percpu;
use crate::serial;
use crate::smp;
use crate::time;

const TRAMPOLINE_PHYSICAL_BASE: u64 = 0x8000;

// Mailbox offsets inside the trampoline page.
const MAILBOX_OFFSET: usize = 0x380;
const MAILBOX_DONE_FLAG: usize = 0x380;
const MAILBOX_CPU_INDEX: usize = 0x384;
const MAILBOX_STACK_PTR: usize = 0x388;
const MAILBOX_PAGE_TABLE: usize = 0x390;
const MAILBOX_RUST_ENTRY: usize = 0x398;
const MAILBOX_PERCPU_BASE: usize = 0x3A0;

// GDT layout offsets (from assembled yasm binary).
// 0x02F0 : gdtr            (.word limit, .long base)
// 0x0300 : null descriptor (8 bytes)
// 0x0308 : 32-bit code     (8 bytes)
// 0x0310 : 32-bit data     (8 bytes)
// 0x0318 : 64-bit code     (8 bytes)
// 0x0320 : 64-bit data     (8 bytes)
const GDTR_BASE_OFFSET: usize = 0x02F2;
const GDT_32BIT_CODE_DESC_OFFSET: usize = 0x0308;
const GDT_32BIT_DATA_DESC_OFFSET: usize = 0x0310;

// Binary blob assembled by build.rs from ap_trampoline.asm.
static AP_TRAMPOLINE_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ap_trampoline.bin"));

#[derive(Copy, Clone)]
pub struct ApBringUpSummary {
    pub attempted: usize,
    pub online: usize,
    pub failed: usize,
}

pub fn bringup_all_aps(hhdm_offset: u64) -> ApBringUpSummary {
    kprintln!("HXNU: ap bringup starting");

    let topology = smp::topology().expect("SMP topology not initialized");
    let mut ap_count = 0usize;
    for cpu in &topology.cpus {
        if cpu.enabled && !cpu.is_bsp {
            ap_count += 1;
        }
    }

    if ap_count == 0 {
        kprintln!("HXNU: no APs to bring up");
        return ApBringUpSummary {
            attempted: 0,
            online: 0,
            failed: 0,
        };
    }

    kprintln!("HXNU: ap bringup targets={}", ap_count);

    // Identity-map the trampoline page so the AP can execute through paging enable.
    kprintln!("HXNU: ap identity-mapping trampoline");
    if let Err(error) = arch::x86_64::map_virtual_page(
        hhdm_offset,
        TRAMPOLINE_PHYSICAL_BASE,
        TRAMPOLINE_PHYSICAL_BASE,
        0,
    ) {
        kprintln!(
            "HXNU: failed to identity-map trampoline page at {:#x}: {:?}",
            TRAMPOLINE_PHYSICAL_BASE,
            error,
        );
        return ApBringUpSummary {
            attempted: ap_count,
            online: 0,
            failed: ap_count,
        };
    }

    let trampoline_size = AP_TRAMPOLINE_BLOB.len();
    assert!(
        trampoline_size <= 4096,
        "trampoline blob {} bytes exceeds page size",
        trampoline_size
    );

    kprintln!("HXNU: ap copying trampoline size={}", trampoline_size);
    unsafe {
        core::ptr::copy_nonoverlapping(
            AP_TRAMPOLINE_BLOB.as_ptr(),
            (hhdm_offset + TRAMPOLINE_PHYSICAL_BASE) as *mut u8,
            trampoline_size,
        );
    }

    // Patch GDT bases inside the copied trampoline.
    let trampoline_virt = hhdm_offset + TRAMPOLINE_PHYSICAL_BASE;
    // GDT physical base = trampoline base + 0x0300.
    let gdt_phys = (TRAMPOLINE_PHYSICAL_BASE + 0x0300) as u32;
    let trampoline_base = TRAMPOLINE_PHYSICAL_BASE as u32;
    kprintln!("HXNU: ap patching gdt phys={:#x} trampoline_base={:#x}", gdt_phys, trampoline_base);
    unsafe {
        use core::ptr::write_volatile;

        // gdtr base points to the GDT entries.
        let gdtr_base_ptr = (trampoline_virt + GDTR_BASE_OFFSET as u64) as *mut u32;
        write_volatile(gdtr_base_ptr, gdt_phys);

        // 32-bit code descriptor: patch base to trampoline_base.
        let code_desc = (trampoline_virt + GDT_32BIT_CODE_DESC_OFFSET as u64) as *mut u8;
        write_volatile(code_desc.add(2), trampoline_base as u8);
        write_volatile(code_desc.add(3), (trampoline_base >> 8) as u8);
        write_volatile(code_desc.add(4), (trampoline_base >> 16) as u8);
        write_volatile(code_desc.add(7), (trampoline_base >> 24) as u8);

        // 32-bit data descriptor: patch base to trampoline_base.
        let data_desc = (trampoline_virt + GDT_32BIT_DATA_DESC_OFFSET as u64) as *mut u8;
        write_volatile(data_desc.add(2), trampoline_base as u8);
        write_volatile(data_desc.add(3), (trampoline_base >> 8) as u8);
        write_volatile(data_desc.add(4), (trampoline_base >> 16) as u8);
        write_volatile(data_desc.add(7), (trampoline_base >> 24) as u8);

    }

    // Read BSP CR3.
    let bsp_cr3: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) bsp_cr3);
    }
    let bsp_cr3_phys = bsp_cr3 & 0x000f_ffff_ffff_f000;

    let mut online = 0usize;
    let mut failed = 0usize;

    for cpu in &topology.cpus {
        if cpu.is_bsp || !cpu.enabled {
            continue;
        }

        kprintln!("HXNU: ap bringing up cpu{} apic={}", cpu.index, cpu.apic_id);

        let stack_frame = match mm::frame::allocate_frame() {
            Some(frame) => frame,
            None => {
                kprintln!("HXNU: failed to allocate stack for AP cpu{}", cpu.index);
                failed += 1;
                continue;
            }
        };
        let stack_phys = stack_frame.start_address();
        let stack_virt = hhdm_offset + stack_phys + mm::frame::PAGE_SIZE;
        let rust_entry = ap_main_rust as *const () as u64;

        let percpu_virt = match percpu::entry_virtual_base(cpu.index) {
            Some(addr) => addr,
            None => {
                kprintln!("HXNU: missing percpu area for AP cpu{}", cpu.index);
                failed += 1;
                continue;
            }
        };

        kprintln!(
            "HXNU: ap mailbox cpu{} stack={:#018x} cr3={:#018x} rust={:#018x} percpu={:#018x}",
            cpu.index,
            stack_virt,
            bsp_cr3_phys,
            rust_entry,
            percpu_virt,
        );

        // Fill mailbox.
        unsafe {
            let mb = trampoline_virt + MAILBOX_OFFSET as u64;
            (mb as *mut AtomicU32).write(AtomicU32::new(0));
            ((mb + (MAILBOX_CPU_INDEX - MAILBOX_OFFSET) as u64) as *mut u32).write(cpu.index as u32);
            ((mb + (MAILBOX_STACK_PTR - MAILBOX_OFFSET) as u64) as *mut u64).write(stack_virt);
            ((mb + (MAILBOX_PAGE_TABLE - MAILBOX_OFFSET) as u64) as *mut u64).write(bsp_cr3_phys);
            ((mb + (MAILBOX_RUST_ENTRY - MAILBOX_OFFSET) as u64) as *mut u64).write(rust_entry);
            ((mb + (MAILBOX_PERCPU_BASE - MAILBOX_OFFSET) as u64) as *mut u64).write(percpu_virt);
        }

        // INIT-SIPI-SIPI sequence.
        serial::write_str("HXNU: ap phase init-assert\n");
        kprintln!("HXNU: ap sending INIT to cpu{} apic={}", cpu.index, cpu.apic_id);
        arch::x86_64::apic::send_init_ipi(cpu.apic_id);
        busy_wait_ns(10_000_000);
        serial::write_str("HXNU: ap phase init-deassert\n");
        arch::x86_64::apic::send_init_ipi_deassert(cpu.apic_id);
        busy_wait_ns(10_000_000);

        let vector = (TRAMPOLINE_PHYSICAL_BASE >> 12) as u8;
        serial::write_str("HXNU: ap phase sipi-1\n");
        kprintln!("HXNU: ap sending SIPI vector={} to cpu{}", vector, cpu.index);
        arch::x86_64::apic::send_startup_ipi(cpu.apic_id, vector);

        let mut ap_done = wait_for_ap_done(trampoline_virt + MAILBOX_DONE_FLAG as u64, 10_000_000);

        if !ap_done {
            serial::write_str("HXNU: ap phase sipi-2\n");
            kprintln!("HXNU: ap sending second SIPI to cpu{}", cpu.index);
            arch::x86_64::apic::send_startup_ipi(cpu.apic_id, vector);
            ap_done = wait_for_ap_done(trampoline_virt + MAILBOX_DONE_FLAG as u64, 50_000_000);
        }

        if ap_done {
            kprintln!("HXNU: AP cpu{} apic={} online", cpu.index, cpu.apic_id);
            smp::mark_cpu_online(cpu.index);
            online += 1;
        } else {
            kprintln!(
                "HXNU: AP cpu{} apic={} failed to boot (timeout)",
                cpu.index,
                cpu.apic_id,
            );
            failed += 1;
        }
    }

    ApBringUpSummary {
        attempted: online + failed,
        online,
        failed,
    }
}
fn wait_for_ap_done(done_flag_addr: u64, timeout_ns: u64) -> bool {
    if !time::is_initialized() {
        let start = read_tsc();
        let timeout_cycles = timeout_ns.max(1);
        while read_tsc().wrapping_sub(start) < timeout_cycles {
            if unsafe { (*(done_flag_addr as *const AtomicU32)).load(Ordering::Acquire) } != 0 {
                return true;
            }
            unsafe {
                asm!("pause", options(nomem, nostack, preserves_flags));
            }
        }
        return false;
    }

    let deadline = time::uptime_nanoseconds().saturating_add(timeout_ns);
    while time::uptime_nanoseconds() < deadline {
        if unsafe { (*(done_flag_addr as *const AtomicU32)).load(Ordering::Acquire) } != 0 {
            return true;
        }
        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }
    false
}

fn busy_wait_ns(ns: u64) {
    if !time::is_initialized() {
        let start = read_tsc();
        let wait_cycles = ns.max(1);
        while read_tsc().wrapping_sub(start) < wait_cycles {
            unsafe {
                asm!("pause", options(nomem, nostack, preserves_flags));
            }
        }
        return;
    }

    let start = time::uptime_nanoseconds();
    while time::uptime_nanoseconds().saturating_sub(start) < ns {
        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }
}

#[inline]
fn read_tsc() -> u64 {
    unsafe { _rdtsc() }
}

#[unsafe(no_mangle)]
extern "C" fn ap_main_rust(cpu_index: u64, percpu_base: u64) {
    let _ = cpu_index;

    // Bind gsbase to the per-CPU area.
    arch::x86_64::cpu::write_msr(0xC000_0101, percpu_base); // IA32_GS_BASE

    arch::x86_64::load_idt();
    serial::write_str("HXNU: AP reached 64-bit Rust\n");

    // For this milestone the AP sits in sti/hlt.
    // Timer and scheduler integration come next.
    loop {
        unsafe {
            asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}
