// HXNU Public License (HPL)
use crate::mm::frame;

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
