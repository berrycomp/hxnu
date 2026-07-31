// TCOL / HPL (HXNU Public License)
// This file is strictly governed by the HXNU Public License (HPL).
use crate::vfs;
use crate::tty::ConsoleStyle;

/// Discovers and loads HX extensions (.hxext) and HX modules (.hxmd) from /boot or /.
/// These custom modules provide critical bare-metal drivers bypassing standard OS APIs.
pub fn load_modules() {
    crate::kprintln_style!(ConsoleStyle::Default, "HXNU: Searching for .hxext and .hxmd modules...");

    let search_paths = ["/boot", "/"];
    let mut modules_found = 0;

    for path in search_paths {
        if let Some(dir_content) = vfs::read(path) {
            for line in dir_content.lines() {
                if line.ends_with(".hxext") || line.ends_with(".hxmd") {
                    modules_found += 1;
                    let full_path = alloc::format!("{}/{}", if path == "/" { "" } else { path }, line);
                    crate::kprintln_style!(ConsoleStyle::Success, "HXNU: Discovered module: {}", full_path);
                    
                    if let Ok(prep) = vfs::prepare_executable_load(&full_path) {
                        if let Some(entry_point) = prep.entry_point {
                            // Map segments properly
                            if let Some(bytes) = vfs::read_executable_bytes(prep.mount, &full_path) {
                                for segment in &prep.vm_map_entries {
                                    let flags = if segment.writable {
                                        crate::arch::x86_64::FLAG_WRITE_THROUGH
                                    } else {
                                        0
                                    };
                                    
                                    let mut page = segment.map_start;
                                    while page < segment.map_end {
                                        if let Some(frame) = crate::mm::frame::allocate_frame() {
                                            let phys = frame.start_address();
                                            let hhdm_offset = crate::limine::hhdm_offset().unwrap();
                                            let virt = hhdm_offset + phys;
                                            
                                            // Zero the frame
                                            unsafe { core::ptr::write_bytes(virt as *mut u8, 0, 4096); }
                                            
                                            // Calculate copy limits
                                            let file_start = segment.virtual_start;
                                            let file_end = file_start + segment.file_bytes;
                                            let page_end = page + 4096;
                                            
                                            let copy_start = page.max(file_start);
                                            let copy_end = page_end.min(file_end);
                                            
                                            if copy_start < copy_end {
                                                let dest_offset = (copy_start - page) as usize;
                                                let src_offset = segment.file_offset as usize + (copy_start - file_start) as usize;
                                                let copy_len = (copy_end - copy_start) as usize;
                                                
                                                if src_offset + copy_len <= bytes.len() {
                                                    unsafe {
                                                        core::ptr::copy_nonoverlapping(
                                                            bytes.as_ptr().add(src_offset),
                                                            (virt as *mut u8).add(dest_offset),
                                                            copy_len,
                                                        );
                                                    }
                                                }
                                            }
                                            
                                            // Map it to kernel page table
                                            let pml4 = crate::arch::x86_64::read_cr3() & 0x000f_ffff_ffff_f000;
                                            unsafe { crate::arch::x86_64::map_kernel_page(hhdm_offset, pml4, page, phys, flags).ok(); }
                                        }
                                        page += 4096;
                                    }
                                }
                            }

                            crate::kprintln_style!(ConsoleStyle::Success, "HXNU: Successfully loaded module {} at entry {:#x}", full_path, entry_point);
                            
                            if full_path.contains("heterexec.hxext") {
                                unsafe {
                                    let init_fn: extern "C" fn() -> fn() = core::mem::transmute(entry_point as usize);
                                    let poll_cycle = init_fn();
                                    crate::sched::HPS_POLL_HOOK = Some(poll_cycle);
                                }
                            }
                        }
                    } else {
                        crate::kprintln_style!(ConsoleStyle::Warning, "HXNU: Module found but failed to load: {}", full_path);
                    }
                }
            }
        }
    }

    if modules_found == 0 {
        crate::kprintln_style!(ConsoleStyle::Warning, "HXNU: No .hxext or .hxmd modules found.");
    } else {
        crate::kprintln_style!(ConsoleStyle::Success, "HXNU: Module loading phase complete. Loaded {} module(s).", modules_found);
    }
}
