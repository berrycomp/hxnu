// THOL (Turkish Hybrid Open License)
// This file is strictly governed by the Turkish Hybrid Open License (THOL).
use crate::mm::frame;
use crate::sched;
use core::cell::UnsafeCell;

#[inline(always)]
fn phys_to_virt(addr: u64) -> *mut u8 {
    (addr + 0xFFFF_8000_0000_0000) as *mut u8
}

pub fn clone_space(parent_pml4: u64) -> u64 {
    let child_pml4 = if let Some(frame) = frame::allocate_frame() {
        frame.start_address()
    } else {
        return 0;
    };

    unsafe {
        core::ptr::copy_nonoverlapping(
            phys_to_virt(parent_pml4).add(256 * 8),
            phys_to_virt(child_pml4).add(256 * 8),
            256 * 8,
        );
        core::ptr::write_bytes(phys_to_virt(child_pml4), 0, 256 * 8);
    }

    for pdpti in 0..256 {
        let parent_pdpt_entry = unsafe { *(phys_to_virt(parent_pml4) as *const u64).offset(pdpti) };
        if parent_pdpt_entry & 1 == 0 { continue; }

        let child_pdpt = frame::allocate_frame().unwrap().start_address();
        unsafe { core::ptr::write_bytes(phys_to_virt(child_pdpt), 0, 4096); }
        unsafe { *(phys_to_virt(child_pml4) as *mut u64).offset(pdpti) = child_pdpt | 7; }

        let parent_pdpt = parent_pdpt_entry & !0xFFF;
        for pdi in 0..512 {
            let parent_pd_entry = unsafe { *(phys_to_virt(parent_pdpt) as *const u64).offset(pdi) };
            if parent_pd_entry & 1 == 0 { continue; }

            let child_pd = frame::allocate_frame().unwrap().start_address();
            unsafe { core::ptr::write_bytes(phys_to_virt(child_pd), 0, 4096); }
            unsafe { *(phys_to_virt(child_pdpt) as *mut u64).offset(pdi) = child_pd | 7; }

            let parent_pd = parent_pd_entry & !0xFFF;
            for pti in 0..512 {
                let parent_pt_entry = unsafe { *(phys_to_virt(parent_pd) as *const u64).offset(pti) };
                if parent_pt_entry & 1 == 0 { continue; }

                let child_pt = frame::allocate_frame().unwrap().start_address();
                unsafe { core::ptr::write_bytes(phys_to_virt(child_pt), 0, 4096); }
                unsafe { *(phys_to_virt(child_pd) as *mut u64).offset(pti) = child_pt | 7; }

                let parent_pt = parent_pt_entry & !0xFFF;
                for fi in 0..512 {
                    let parent_frame_entry = unsafe { *(phys_to_virt(parent_pt) as *const u64).offset(fi) };
                    if parent_frame_entry & 1 == 0 { continue; }

                    let child_frame = frame::allocate_frame().unwrap().start_address();
                    let parent_frame = parent_frame_entry & !0xFFF;
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            phys_to_virt(parent_frame),
                            phys_to_virt(child_frame),
                            4096,
                        );
                        *(phys_to_virt(child_pt) as *mut u64).offset(fi) = child_frame | (parent_frame_entry & 0xFFF);
                    }
                }
            }
        }
    }

    child_pml4
}
const MAX_PROCESSES: usize = 64;
const MAX_REGIONS_PER_PROCESS: usize = 128;

#[derive(Copy, Clone)]
pub struct NativeRegion {
    pub start: u64,
    pub end: u64,
    pub flags: u64,
    pub used: bool,
}

#[derive(Copy, Clone)]
pub struct ProcessVmm {
    pub process_id: u64,
    pub active: bool,
    pub regions: [NativeRegion; MAX_REGIONS_PER_PROCESS],
}

struct GlobalVmmTable(UnsafeCell<[ProcessVmm; MAX_PROCESSES]>);
unsafe impl Sync for GlobalVmmTable {}

impl GlobalVmmTable {
    const fn new() -> Self {
        Self(UnsafeCell::new([ProcessVmm {
            process_id: 0,
            active: false,
            regions: [NativeRegion { start: 0, end: 0, flags: 0, used: false }; MAX_REGIONS_PER_PROCESS],
        }; MAX_PROCESSES]))
    }
}

static VMM_TABLE: GlobalVmmTable = GlobalVmmTable::new();

pub fn get_process_vmm(pid: u64) -> &'static mut ProcessVmm {
    let table = unsafe { &mut *VMM_TABLE.0.get() };
    for i in 0..MAX_PROCESSES {
        if table[i].active && table[i].process_id == pid {
            return &mut table[i];
        }
    }
    
    // Allocate new
    for i in 0..MAX_PROCESSES {
        if !table[i].active {
            table[i].active = true;
            table[i].process_id = pid;
            for r in table[i].regions.iter_mut() {
                r.used = false;
            }
            return &mut table[i];
        }
    }
    panic!("Out of VMM slots");
}

pub fn map_native_region(addr: u64, len: u64, flags: u64) -> i64 {
    let pid = sched::current_process_id();
    let vmm = get_process_vmm(pid);
    let aligned_len = (len + 4095) & !4095;
    
    let map_addr = if addr == 0 {
        let mut try_addr = 0x4000_0000_0000;
        loop {
            let mut overlap = false;
            for r in vmm.regions.iter() {
                if r.used && r.start < try_addr + aligned_len && r.end > try_addr {
                    overlap = true;
                    try_addr = r.end;
                    break;
                }
            }
            if !overlap {
                break try_addr;
            }
        }
    } else {
        addr & !4095
    };

    for r in vmm.regions.iter_mut() {
        if !r.used {
            r.used = true;
            r.start = map_addr;
            r.end = map_addr + aligned_len;
            r.flags = flags;
            return map_addr as i64;
        }
    }
    -12 // ENOMEM
}

pub fn handle_page_fault(cr2: u64, _error_code: u64) -> bool {
    let pid = sched::current_process_id();
    let vmm = get_process_vmm(pid);
    
    for r in vmm.regions.iter() {
        if r.used && cr2 >= r.start && cr2 < r.end {
            return map_page(cr2, r.flags);
        }
    }
    false
}

fn map_page(vaddr: u64, flags: u64) -> bool {
    let pml4 = unsafe {
        let mut cr3: u64;
        core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack));
        cr3 & !0xFFF
    };
    
    let pml4_idx = (vaddr >> 39) & 0x1FF;
    let pdpt_idx = (vaddr >> 30) & 0x1FF;
    let pd_idx = (vaddr >> 21) & 0x1FF;
    let pt_idx = (vaddr >> 12) & 0x1FF;
    
    let pml4_entry = unsafe { &mut *(phys_to_virt(pml4) as *mut u64).offset(pml4_idx as isize) };
    if (*pml4_entry & 1) == 0 {
        if let Some(frame) = frame::allocate_frame() {
            let addr = frame.start_address();
            unsafe { core::ptr::write_bytes(phys_to_virt(addr), 0, 4096); }
            *pml4_entry = addr | 7;
        } else {
            return false;
        }
    }
    
    let pdpt = *pml4_entry & !0xFFF;
    let pdpt_entry = unsafe { &mut *(phys_to_virt(pdpt) as *mut u64).offset(pdpt_idx as isize) };
    if (*pdpt_entry & 1) == 0 {
        if let Some(frame) = frame::allocate_frame() {
            let addr = frame.start_address();
            unsafe { core::ptr::write_bytes(phys_to_virt(addr), 0, 4096); }
            *pdpt_entry = addr | 7;
        } else {
            return false;
        }
    }
    
    let pd = *pdpt_entry & !0xFFF;
    let pd_entry = unsafe { &mut *(phys_to_virt(pd) as *mut u64).offset(pd_idx as isize) };
    if (*pd_entry & 1) == 0 {
        if let Some(frame) = frame::allocate_frame() {
            let addr = frame.start_address();
            unsafe { core::ptr::write_bytes(phys_to_virt(addr), 0, 4096); }
            *pd_entry = addr | 7;
        } else {
            return false;
        }
    }
    
    let pt = *pd_entry & !0xFFF;
    let pt_entry = unsafe { &mut *(phys_to_virt(pt) as *mut u64).offset(pt_idx as isize) };
    if (*pt_entry & 1) == 0 {
        if let Some(frame) = frame::allocate_frame() {
            let addr = frame.start_address();
            unsafe { core::ptr::write_bytes(phys_to_virt(addr), 0, 4096); }
            *pt_entry = addr | flags;
        } else {
            return false;
        }
    }
    
    true
}
