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
                            // Map or execute module
                            crate::kprintln_style!(ConsoleStyle::Success, "HXNU: Successfully loaded module {} at entry {:#x}", full_path, entry_point);
                            
                            // A real dynamic loader would allocate, map segments, and perform relocations.
                            // For this skeleton, we just log it and potentially call it if it's the HPS bridge.
                            if full_path.contains("heterexec.hxext") {
                                // Simulate hooking
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
