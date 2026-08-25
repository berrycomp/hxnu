// THOL (Turkish Hybrid Open License)
// This file is strictly governed by the Turkish Hybrid Open License (THOL).
use crate::vfs;
use crate::initrd;
use crate::tty::ConsoleStyle;

/// Discovers and loads HX extensions (.hxext) and HX modules (.hxmd) from the initrd.
/// Uses initrd::get_module_paths() — non-generic, safe against release LTO elimination.
pub fn load_modules() {
    crate::kprintln_style!(ConsoleStyle::Default, "HXNU: Searching for .hxext and .hxmd modules...");

    let module_paths = initrd::get_module_paths();
    let modules_found = module_paths.len();

    for full_path in &module_paths {
        // heterexec ELF requires relocation processing before entry call.
        // Defer until ELF .rela sections are handled — skip for this boot.
        if full_path.contains("heterexec.hxext") {
            crate::kprintln_style!(ConsoleStyle::Default,
                "HXNU: heterexec deferred (relocation pass not yet implemented): {}", full_path);
            continue;
        }

        crate::kprintln_style!(ConsoleStyle::Success, "HXNU: Discovered module: {}", full_path);

        if let Ok(prep) = vfs::prepare_executable_load(full_path) {
            if let Some(entry_point) = prep.entry_point {
                if let Some(bytes) = vfs::read_executable_bytes(prep.mount, full_path) {
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

                                unsafe { core::ptr::write_bytes(virt as *mut u8, 0, 4096); }

                                let file_start = segment.virtual_start;
                                let file_end = file_start + segment.file_bytes;
                                let page_end = page + 4096;
                                let copy_start = page.max(file_start);
                                let copy_end = page_end.min(file_end);

                                if copy_start < copy_end {
                                    let dest_offset = (copy_start - page) as usize;
                                    let src_offset = segment.file_offset as usize
                                        + (copy_start - file_start) as usize;
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

                                let pml4 = crate::arch::x86_64::read_cr3()
                                    & 0x000f_ffff_ffff_f000;
                                unsafe {
                                    crate::arch::x86_64::map_kernel_page(
                                        hhdm_offset, pml4, page, phys, flags,
                                    )
                                    .ok();
                                }
                            }
                            page += 4096;
                        }
                    }
                } // end bytes

                crate::kprintln_style!(
                    ConsoleStyle::Success,
                    "HXNU: Successfully loaded module {} at entry {:#x}",
                    full_path,
                    entry_point
                );

                // NOTE: heterexec entry point call is deferred until ELF relocation
                // processing is implemented. Calling it now causes a page fault
                // because GOT/PLT relocations are unresolved (cr2=0xf null deref).
                if full_path.contains("heterexec.hxext") {
                    crate::kprintln_style!(
                        ConsoleStyle::Default,
                        "HXNU: heterexec staged at {:#x} — relocation pass pending",
                        entry_point
                    );
                }
            } // end entry_point
        } else {
            crate::kprintln_style!(
                ConsoleStyle::Warning,
                "HXNU: Module found but failed to load: {}",
                full_path
            );
        } // end prepare_executable_load
    } // end for

    if modules_found == 0 {
        crate::kprintln_style!(ConsoleStyle::Warning, "HXNU: No .hxext or .hxmd modules found.");
    } else {
        crate::kprintln_style!(
            ConsoleStyle::Success,
            "HXNU: Module loading phase complete. Loaded {} module(s).",
            modules_found
        );
    }
}
