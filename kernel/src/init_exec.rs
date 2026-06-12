use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::fmt::Write;

use crate::arch;
use crate::initrd;
use crate::mm;
use crate::sched;
use crate::vfs;
use crate::vfs::{ExecutableFormat, VmMapPlanEntry};

struct GlobalInitExec(UnsafeCell<InitExecState>);

unsafe impl Sync for GlobalInitExec {}

impl GlobalInitExec {
    const fn new() -> Self {
        Self(UnsafeCell::new(InitExecState::new()))
    }

    fn get(&self) -> *mut InitExecState {
        self.0.get()
    }
}

static INIT_EXEC: GlobalInitExec = GlobalInitExec::new();
const MAX_INIT_RESTARTS: u32 = 4;

struct InitExecState {
    activation: Option<ActivatedInitImage>,
    last_error: Option<InitExecActivateError>,
    current_process_id: u64,
    current_thread_id: u64,
    restart_pending: bool,
    restart_count: u32,
    last_exit_status: Option<i32>,
}

impl InitExecState {
    const fn new() -> Self {
        Self {
            activation: None,
            last_error: None,
            current_process_id: 0,
            current_thread_id: 0,
            restart_pending: false,
            restart_count: 0,
            last_exit_status: None,
        }
    }
}

struct ActivatedInitImage {
    path: String,
    format: ExecutableFormat,
    image_type: u16,
    machine: u16,
    entry_point: u64,
    vm_start: u64,
    vm_end: u64,
    total_bytes: u64,
    zero_fill_bytes: u64,
    entry_segment_index: usize,
    entry_segment_map_offset: u64,
    segments: Vec<ActivatedSegment>,
}

struct ActivatedSegment {
    index: usize,
    virtual_start: u64,
    virtual_end: u64,
    map_start: u64,
    map_end: u64,
    file_bytes: u64,
    memory_bytes: u64,
    readable: bool,
    writable: bool,
    executable: bool,
    bytes: Vec<u8>,
}

#[derive(Copy, Clone)]
pub struct InitExecSummary {
    pub armed: bool,
    pub format: ExecutableFormat,
    pub image_type: u16,
    pub machine: u16,
    pub entry_point: u64,
    pub segment_count: usize,
    pub total_bytes: u64,
    pub zero_fill_bytes: u64,
    pub vm_start: u64,
    pub vm_end: u64,
    pub entry_segment_index: usize,
    pub entry_segment_map_offset: u64,
}

#[derive(Copy, Clone)]
pub struct SpawnedInitProcess {
    pub thread_id: u64,
    pub process_id: u64,
    pub restart_count: u32,
}

#[derive(Copy, Clone)]
pub struct InitExitDisposition {
    pub process_id: u64,
    pub thread_id: u64,
    pub exit_status: i32,
    pub restart_scheduled: bool,
    pub next_restart_attempt: u32,
    pub restart_limit: u32,
}

#[derive(Copy, Clone)]
pub struct RestartedInitProcess {
    pub thread_id: u64,
    pub process_id: u64,
    pub restart_count: u32,
    pub last_exit_status: i32,
}

#[derive(Copy, Clone)]
pub enum InitExecActivateError {
    Load(vfs::ExecutableLoadPrepError),
    BytesUnavailable,
    UnsupportedFormat,
    MissingEntryPoint,
    MissingImageType,
    MissingMachine,
    NoLoadSegments,
    NoExecutableSegments,
    EntryOutsideExecutableSegments,
    InvalidSegmentMap,
}

impl InitExecActivateError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Load(error) => error.as_str(),
            Self::BytesUnavailable => "init executable bytes not available",
            Self::UnsupportedFormat => "init executable format is not ELF",
            Self::MissingEntryPoint => "init executable has no entry point",
            Self::MissingImageType => "init executable is missing ELF image type",
            Self::MissingMachine => "init executable is missing ELF machine id",
            Self::NoLoadSegments => "init executable has no loadable segments",
            Self::NoExecutableSegments => "init executable has no executable load segment",
            Self::EntryOutsideExecutableSegments => "entry point is outside executable segments",
            Self::InvalidSegmentMap => "materialized segment map is invalid",
        }
    }
}

pub fn activate_init_handoff() -> Result<InitExecSummary, InitExecActivateError> {
    let result = (|| {
        let image = vfs::prepare_init_load().map_err(InitExecActivateError::Load)?;
        let bytes = initrd::read_bytes("/initrd/init")
            .ok_or(InitExecActivateError::BytesUnavailable)?;
        build_activation(image, bytes)
    })();

    let state = state_mut();
    match result {
        Ok(activation) => {
            let summary = activation_summary(&activation);
            state.activation = Some(activation);
            state.last_error = None;
            state.current_process_id = 0;
            state.current_thread_id = 0;
            state.restart_pending = false;
            state.restart_count = 0;
            state.last_exit_status = None;
            Ok(summary)
        }
        Err(error) => {
            state.activation = None;
            state.last_error = Some(error);
            state.current_process_id = 0;
            state.current_thread_id = 0;
            state.restart_pending = false;
            state.restart_count = 0;
            state.last_exit_status = None;
            Err(error)
        }
    }
}

pub fn render_status() -> String {
    let state = state_ref();
    let mut text = String::new();

    match state.activation.as_ref() {
        Some(activation) => {
            let summary = activation_summary(activation);
            let _ = writeln!(text, "armed {}", yes_no(summary.armed));
            let _ = writeln!(text, "path {}", activation.path);
            let _ = writeln!(text, "format {}", summary.format.as_str());
            let _ = writeln!(text, "machine {:#06x}", summary.machine);
            let _ = writeln!(text, "image_type {:#06x}", summary.image_type);
            let _ = writeln!(text, "entry_point {:#018x}", summary.entry_point);
            let _ = writeln!(
                text,
                "vm_range {:#018x}..{:#018x}",
                summary.vm_start,
                summary.vm_end
            );
            let _ = writeln!(text, "segments {}", summary.segment_count);
            let _ = writeln!(text, "bytes {}", summary.total_bytes);
            let _ = writeln!(text, "zero_fill {}", summary.zero_fill_bytes);
            let _ = writeln!(text, "entry_segment {}", summary.entry_segment_index);
            let _ = writeln!(text, "entry_offset {}", summary.entry_segment_map_offset);
            if let Some(segment) = activation.segments.first() {
                let _ = writeln!(
                    text,
                    "segment0 idx={} vaddr={:#018x}..{:#018x} map={:#018x}..{:#018x} file={} mem={} perms={}{}{} bytes={}",
                    segment.index,
                    segment.virtual_start,
                    segment.virtual_end,
                    segment.map_start,
                    segment.map_end,
                    segment.file_bytes,
                    segment.memory_bytes,
                    if segment.readable { 'r' } else { '-' },
                    if segment.writable { 'w' } else { '-' },
                    if segment.executable { 'x' } else { '-' },
                    segment.bytes.len(),
                );
            }
        }
        None => {
            let _ = writeln!(text, "armed no");
        }
    }

    let last_error = state.last_error.map(|error| error.as_str()).unwrap_or("<none>");
    let _ = writeln!(text, "last_error {}", last_error);
    text
}

const USER_SPACE_BASE: u64 = 0x0000_0000_0040_0000;
const USER_SPACE_LIMIT: u64 = 0x0000_8000_0000_0000;

fn build_activation(
    image: vfs::ExecutableLoadPrep,
    bytes: &'static [u8],
) -> Result<ActivatedInitImage, InitExecActivateError> {
    if image.format != ExecutableFormat::Elf {
        return Err(InitExecActivateError::UnsupportedFormat);
    }

    let entry_point = image.entry_point.ok_or(InitExecActivateError::MissingEntryPoint)?;
    let image_type = image.image_type.ok_or(InitExecActivateError::MissingImageType)?;
    let machine = image.machine.ok_or(InitExecActivateError::MissingMachine)?;
    if image.vm_map_entries.is_empty() {
        return Err(InitExecActivateError::NoLoadSegments);
    }

    let mut orig_vm_start = u64::MAX;
    let mut orig_vm_end = 0u64;
    for segment in &image.vm_map_entries {
        orig_vm_start = orig_vm_start.min(segment.map_start);
        orig_vm_end = orig_vm_end.max(segment.map_end);
    }
    let user_load_base = if orig_vm_start < USER_SPACE_LIMIT {
        orig_vm_start
    } else {
        USER_SPACE_BASE
    };

    let mut has_executable_segment = false;
    let mut entry_segment_index = None;
    let mut entry_segment_map_offset = 0u64;
    let mut segments = Vec::with_capacity(image.vm_map_entries.len());

    for segment in image.vm_map_entries.into_iter() {
        let file_offset = usize::try_from(segment.file_offset)
            .map_err(|_| InitExecActivateError::InvalidSegmentMap)?;
        let file_bytes_usize = usize::try_from(segment.file_bytes)
            .map_err(|_| InitExecActivateError::InvalidSegmentMap)?;
        let segment_bytes = if file_offset + file_bytes_usize <= bytes.len() {
            bytes[file_offset..file_offset + file_bytes_usize].to_vec()
        } else {
            return Err(InitExecActivateError::InvalidSegmentMap);
        };

        let expected_len = segment.file_bytes;
        let actual_len = u64::try_from(segment_bytes.len())
            .map_err(|_| InitExecActivateError::InvalidSegmentMap)?;
        if expected_len != actual_len {
            return Err(InitExecActivateError::InvalidSegmentMap);
        }

        let offset = segment.map_start.checked_sub(orig_vm_start)
            .ok_or(InitExecActivateError::InvalidSegmentMap)?;
        let user_map_start = user_load_base.checked_add(offset)
            .ok_or(InitExecActivateError::InvalidSegmentMap)?;
        let user_map_end = user_map_start.checked_add(segment.map_end - segment.map_start)
            .ok_or(InitExecActivateError::InvalidSegmentMap)?;
        let user_virtual_start = user_load_base.checked_add(
            segment.virtual_start.checked_sub(orig_vm_start)
                .ok_or(InitExecActivateError::InvalidSegmentMap)?,
        ).ok_or(InitExecActivateError::InvalidSegmentMap)?;
        let user_virtual_end = user_load_base.checked_add(
            segment.virtual_end.checked_sub(orig_vm_start)
                .ok_or(InitExecActivateError::InvalidSegmentMap)?,
        ).ok_or(InitExecActivateError::InvalidSegmentMap)?;

        if segment.executable {
            has_executable_segment = true;
            if entry_point >= segment.virtual_start && entry_point < segment.virtual_end {
                entry_segment_index = Some(segment.index);
                entry_segment_map_offset = entry_point
                    .checked_sub(segment.map_start)
                    .ok_or(InitExecActivateError::InvalidSegmentMap)?;
            }
        }

        segments.push(ActivatedSegment {
            index: segment.index,
            virtual_start: user_virtual_start,
            virtual_end: user_virtual_end,
            map_start: user_map_start,
            map_end: user_map_end,
            file_bytes: segment.file_bytes,
            memory_bytes: segment.memory_bytes,
            readable: segment.readable,
            writable: segment.writable,
            executable: segment.executable,
            bytes: segment_bytes,
        });
    }

    if !has_executable_segment {
        return Err(InitExecActivateError::NoExecutableSegments);
    }
    let entry_segment_index = entry_segment_index.ok_or(InitExecActivateError::EntryOutsideExecutableSegments)?;

    let user_entry = user_load_base.checked_add(
        entry_point.checked_sub(orig_vm_start)
            .ok_or(InitExecActivateError::InvalidSegmentMap)?,
    ).ok_or(InitExecActivateError::InvalidSegmentMap)?;

    let vm_size = orig_vm_end.checked_sub(orig_vm_start)
        .ok_or(InitExecActivateError::InvalidSegmentMap)?;
    let user_vm_end = user_load_base.checked_add(vm_size)
        .ok_or(InitExecActivateError::InvalidSegmentMap)?;

    Ok(ActivatedInitImage {
        path: image.path,
        format: image.format,
        image_type,
        machine,
        entry_point: user_entry,
        vm_start: user_load_base,
        vm_end: user_vm_end,
        total_bytes: image.vm_map_total_bytes,
        zero_fill_bytes: image.vm_map_zero_fill_bytes,
        entry_segment_index,
        entry_segment_map_offset,
        segments,
    })
}

fn activation_summary(activation: &ActivatedInitImage) -> InitExecSummary {
    InitExecSummary {
        armed: true,
        format: activation.format,
        image_type: activation.image_type,
        machine: activation.machine,
        entry_point: activation.entry_point,
        segment_count: activation.segments.len(),
        total_bytes: activation.total_bytes,
        zero_fill_bytes: activation.zero_fill_bytes,
        vm_start: activation.vm_start,
        vm_end: activation.vm_end,
        entry_segment_index: activation.entry_segment_index,
        entry_segment_map_offset: activation.entry_segment_map_offset,
    }
}

fn state_ref() -> &'static InitExecState {
    unsafe { &*INIT_EXEC.get() }
}

fn state_mut() -> &'static mut InitExecState {
    unsafe { &mut *INIT_EXEC.get() }
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

pub fn spawn_init_process() -> Result<SpawnedInitProcess, InitExecActivateError> {
    let state = state_ref();
    let activation = state.activation.as_ref().ok_or(InitExecActivateError::BytesUnavailable)?;

    let hhdm_offset = crate::limine::hhdm_offset().unwrap();

    let user_pml4 = arch::x86_64::create_user_page_table(hhdm_offset)
        .map_err(|_| InitExecActivateError::InvalidSegmentMap)?;

    for segment in &activation.segments {
        let flags = if segment.writable {
            arch::x86_64::FLAG_USER_ACCESSIBLE | arch::x86_64::FLAG_WRITE_THROUGH
        } else {
            arch::x86_64::FLAG_USER_ACCESSIBLE
        };

        map_segment_pages(
            segment,
            user_pml4,
            hhdm_offset,
            flags,
        )?;
    }

    let user_stack_top = 0x0000_7fff_ffff_0000u64;
    let user_stack_size = 4096usize;
    let user_stack_phys = mm::frame::allocate_frame()
        .ok_or(InitExecActivateError::InvalidSegmentMap)?
        .start_address();

    arch::x86_64::map_user_region(
        user_pml4,
        hhdm_offset,
        user_stack_top - user_stack_size as u64,
        user_stack_phys,
        user_stack_size,
        arch::x86_64::FLAG_USER_ACCESSIBLE | arch::x86_64::FLAG_WRITE_THROUGH,
    ).map_err(|_| InitExecActivateError::InvalidSegmentMap)?;

    let spawned = sched::create_user_thread(
        activation.entry_point,
        user_stack_top,
        user_pml4,
    ).map_err(|_| InitExecActivateError::InvalidSegmentMap)?;

    let state = state_mut();
    state.current_process_id = spawned.process_id;
    state.current_thread_id = spawned.thread_id;
    state.restart_pending = false;

    Ok(SpawnedInitProcess {
        thread_id: spawned.thread_id,
        process_id: spawned.process_id,
        restart_count: state.restart_count,
    })
}

pub fn note_init_exit(
    process_id: u64,
    thread_id: u64,
    exit_status: i32,
) -> Option<InitExitDisposition> {
    let state = state_mut();
    if state.current_process_id == 0 || state.current_process_id != process_id {
        return None;
    }

    state.current_process_id = 0;
    state.current_thread_id = 0;
    state.last_exit_status = Some(exit_status);
    state.restart_pending = state.restart_count < MAX_INIT_RESTARTS;

    Some(InitExitDisposition {
        process_id,
        thread_id,
        exit_status,
        restart_scheduled: state.restart_pending,
        next_restart_attempt: state.restart_count.saturating_add(1),
        restart_limit: MAX_INIT_RESTARTS,
    })
}

pub fn service_pending_restart() -> Option<Result<RestartedInitProcess, InitExecActivateError>> {
    let last_exit_status;
    {
        let state = state_mut();
        if !state.restart_pending {
            return None;
        }

        state.restart_pending = false;
        state.restart_count = state.restart_count.saturating_add(1);
        last_exit_status = state.last_exit_status.unwrap_or(0);
    }

    Some(spawn_init_process().map(|spawned| RestartedInitProcess {
        thread_id: spawned.thread_id,
        process_id: spawned.process_id,
        restart_count: spawned.restart_count,
        last_exit_status,
    }))
}

fn map_segment_pages(
    segment: &ActivatedSegment,
    user_pml4: u64,
    hhdm_offset: u64,
    flags: u64,
) -> Result<(), InitExecActivateError> {
    let file_start = segment.virtual_start;
    let file_end = file_start
        .checked_add(u64::try_from(segment.bytes.len()).map_err(|_| InitExecActivateError::InvalidSegmentMap)?)
        .ok_or(InitExecActivateError::InvalidSegmentMap)?;

    let mut page = segment.map_start;
    while page < segment.map_end {
        let frame = mm::frame::allocate_frame()
            .ok_or(InitExecActivateError::InvalidSegmentMap)?;
        let phys = frame.start_address();
        let virt = hhdm_offset
            .checked_add(phys)
            .ok_or(InitExecActivateError::InvalidSegmentMap)?;

        unsafe {
            core::ptr::write_bytes(virt as *mut u8, 0, mm::frame::PAGE_SIZE as usize);
        }

        let page_end = page
            .checked_add(mm::frame::PAGE_SIZE)
            .ok_or(InitExecActivateError::InvalidSegmentMap)?;
        let copy_start = page.max(file_start);
        let copy_end = page_end.min(file_end);
        if copy_start < copy_end {
            let dest_offset = usize::try_from(copy_start - page)
                .map_err(|_| InitExecActivateError::InvalidSegmentMap)?;
            let src_offset = usize::try_from(copy_start - file_start)
                .map_err(|_| InitExecActivateError::InvalidSegmentMap)?;
            let copy_len = usize::try_from(copy_end - copy_start)
                .map_err(|_| InitExecActivateError::InvalidSegmentMap)?;

            unsafe {
                core::ptr::copy_nonoverlapping(
                    segment.bytes.as_ptr().add(src_offset),
                    (virt as *mut u8).add(dest_offset),
                    copy_len,
                );
            }
        }

        arch::x86_64::map_user_region(
            user_pml4,
            hhdm_offset,
            page,
            phys,
            mm::frame::PAGE_SIZE as usize,
            flags,
        ).map_err(|_| InitExecActivateError::InvalidSegmentMap)?;

        page = page_end;
    }

    Ok(())
}
