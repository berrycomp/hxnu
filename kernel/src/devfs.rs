// THOL (Turkish Hybrid Open License)
// This file is strictly governed by the Turkish Hybrid Open License (THOL).
use alloc::string::String;
use core::cell::UnsafeCell;
use core::fmt::Write;

use crate::tty;

const DEVFS_DIRECTORIES: [&str; 2] = ["/", "/dev"];
const DEVFS_NODES: [&str; 9] = [
    "/dev/console",
    "/dev/tty0",
    "/dev/tty1",
    "/dev/tty2",
    "/dev/tty3",
    "/dev/null",
    "/dev/zero",
    "/dev/kmsg",
    "/dev/fb0",
];

struct GlobalDevfs(UnsafeCell<Option<DevfsState>>);

unsafe impl Sync for GlobalDevfs {}

impl GlobalDevfs {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<DevfsState> {
        self.0.get()
    }
}

static DEVFS: GlobalDevfs = GlobalDevfs::new();

#[derive(Copy, Clone)]
struct DevfsState {
    boot_console_id: u32,
    boot_output_count: u8,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DevfsNodeKind {
    Directory,
    Device,
}

#[derive(Copy, Clone)]
pub struct DevfsSummary {
    pub directory_count: usize,
    pub node_count: usize,
    pub entry_count: usize,
}

#[derive(Copy, Clone)]
pub struct DevfsMmapInfo {
    pub physical_address: u64,
    pub size: usize,
}

#[derive(Copy, Clone)]
pub enum DevfsError {
    AlreadyInitialized,
}

impl DevfsError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "devfs is already initialized",
        }
    }
}

pub fn initialize() -> Result<DevfsSummary, DevfsError> {
    let slot = unsafe { &mut *DEVFS.get() };
    if slot.is_some() {
        return Err(DevfsError::AlreadyInitialized);
    }

    let tty = tty::stats();
    *slot = Some(DevfsState {
        boot_console_id: tty.console_id,
        boot_output_count: tty.output_count,
    });

    Ok(summary())
}

pub fn summary() -> DevfsSummary {
    DevfsSummary {
        directory_count: DEVFS_DIRECTORIES.len(),
        node_count: DEVFS_NODES.len(),
        entry_count: DEVFS_DIRECTORIES.len() + DEVFS_NODES.len(),
    }
}

pub fn node_kind(path: &str) -> Option<DevfsNodeKind> {
    match path {
        "/dev" | "/dev/" => Some(DevfsNodeKind::Directory),
        _ if DEVFS_NODES.iter().any(|node| *node == path) => Some(DevfsNodeKind::Device),
        _ => None,
    }
}

#[allow(dead_code)]
pub fn mmap(path: &str) -> Option<DevfsMmapInfo> {
    match path {
        "/dev/fb0" => {
            let fb = crate::fb::summary()?;
            let physical_address = crate::fb::physical_address()?;
            let size = (fb.pitch * fb.height) as usize;
            Some(DevfsMmapInfo {
                physical_address,
                size,
            })
        }
        _ => None,
    }
}

pub fn read(path: &str) -> Option<String> {
    let state = unsafe { (&*DEVFS.get()).as_ref()? };
    match path {
        "/dev" | "/dev/" => Some(render_root()),
        "/dev/console" => Some(render_console(state, "/dev/console")),
        "/dev/tty0" => Some(render_console(state, "/dev/tty0")),
        "/dev/tty1" => Some(render_console(state, "/dev/tty1")),
        "/dev/tty2" => Some(render_console(state, "/dev/tty2")),
        "/dev/tty3" => Some(render_console(state, "/dev/tty3")),
        "/dev/null" => Some(render_null()),
        "/dev/zero" => Some(render_zero()),
        "/dev/kmsg" => Some(render_kmsg()),
        "/dev/fb0" => Some(render_fb0()),
        _ => None,
    }
}

fn render_root() -> String {
    let mut text = String::new();
    for node in DEVFS_NODES {
        let _ = writeln!(text, "{}", node.trim_start_matches("/dev/"));
    }
    text
}

fn render_console(state: &DevfsState, path: &str) -> String {
    let mut text = String::new();
    let tty_stats = tty::stats();
    let _ = writeln!(text, "path {}", path);
    let _ = writeln!(text, "kind tty-console");
    let _ = writeln!(text, "console_id {}", tty_stats.console_id);
    let _ = writeln!(text, "outputs {}", tty_stats.output_count);
    let _ = writeln!(text, "bytes {}", tty_stats.bytes_written);
    let _ = writeln!(text, "lines {}", tty_stats.lines_written);
    let _ = writeln!(text, "boot_console_id {}", state.boot_console_id);
    let _ = writeln!(text, "boot_outputs {}", state.boot_output_count);
    text
}

fn render_null() -> String {
    let mut text = String::new();
    let _ = writeln!(text, "path /dev/null");
    let _ = writeln!(text, "kind sink");
    let _ = writeln!(text, "reads eof");
    let _ = writeln!(text, "writes discard");
    text
}

fn render_zero() -> String {
    let mut text = String::new();
    let _ = writeln!(text, "path /dev/zero");
    let _ = writeln!(text, "kind source");
    let _ = writeln!(text, "reads zero-fill");
    let _ = writeln!(text, "writes discard");
    text
}

fn render_kmsg() -> String {
    let mut text = String::new();
    let _ = writeln!(text, "path /dev/kmsg");
    let _ = writeln!(text, "kind kernel-log");
    let _ = writeln!(text, "writes append");
    let _ = writeln!(text, "reads snapshot-unavailable");
    text
}

fn render_fb0() -> String {
    let mut text = String::new();
    let _ = writeln!(text, "path /dev/fb0");
    let _ = writeln!(text, "kind framebuffer");
    if let Some(fb) = crate::fb::summary() {
        let _ = writeln!(text, "width {}", fb.width);
        let _ = writeln!(text, "height {}", fb.height);
        let _ = writeln!(text, "pitch {}", fb.pitch);
        let _ = writeln!(text, "bpp {}", fb.bpp);
    }
    if let Some(phys) = crate::fb::physical_address() {
        let _ = writeln!(text, "phys_addr {:#x}", phys);
    }
    text
}
