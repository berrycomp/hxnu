use alloc::string::String;
use alloc::vec::Vec;
use core::arch::asm;
use core::cell::UnsafeCell;
use core::fmt::Write;
use core::ptr;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::arch;
use crate::kprintln;
use crate::limine;
use crate::mm;
use crate::syscall;
use crate::vfs;
use crate::vfs::{ExecutableFormat, VmMapImageEntry};

struct GlobalInitExec(UnsafeCell<InitExecState>);

unsafe impl Sync for GlobalInitExec {}

const PAGE_BYTES: usize = mm::frame::PAGE_SIZE as usize;
const PAGE_MASK: u64 = mm::frame::PAGE_SIZE - 1;
const ELF_MACHINE_X86_64: u16 = 0x003e;
const INIT_LAUNCH_MONITOR_TICKS: u32 = 3;
const BOOTSTRAP_STACK_RESERVE_BYTES: u64 = 64 * 1024;

impl GlobalInitExec {
    const fn new() -> Self {
        Self(UnsafeCell::new(InitExecState::new()))
    }

    fn get(&self) -> *mut InitExecState {
        self.0.get()
    }
}

static INIT_EXEC: GlobalInitExec = GlobalInitExec::new();
static LAUNCH_MONITOR_BUDGET: AtomicU32 = AtomicU32::new(0);

struct InitExecState {
    activation: Option<ActivatedInitImage>,
    last_error: Option<InitExecActivateError>,
    last_launch_error: Option<InitExecLaunchError>,
    active_launch: Option<ActiveLaunch>,
}

impl InitExecState {
    const fn new() -> Self {
        Self {
            activation: None,
            last_error: None,
            last_launch_error: None,
            active_launch: None,
        }
    }
}

#[derive(Clone)]
struct ActiveLaunch {
    process_id: u64,
    path: String,
    pages: Vec<TrackedPage>,
}

#[derive(Copy, Clone)]
struct TrackedPage {
    virtual_address: u64,
    physical_address: u64,
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
pub enum InitExecActivateError {
    Load(vfs::ExecutableLoadPrepError),
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

#[derive(Copy, Clone)]
pub enum InitExecLaunchError {
    Activation(InitExecActivateError),
    UnsupportedMachine,
    Prep(vfs::ExecutableLoadPrepError),
    MissingInterpreter,
    MissingHhdm,
    StackRangeInvalid,
    SegmentRangeInvalid,
    FrameAllocationFailed,
    Map(arch::x86_64::MapError),
}

impl InitExecLaunchError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Activation(error) => error.as_str(),
            Self::UnsupportedMachine => "init executable machine is not supported for launch",
            Self::Prep(error) => error.as_str(),
            Self::MissingInterpreter => "exec interpreter could not be resolved",
            Self::MissingHhdm => "limine HHDM offset is not available",
            Self::StackRangeInvalid => "init bootstrap stack range is invalid",
            Self::SegmentRangeInvalid => "init executable segment range is invalid",
            Self::FrameAllocationFailed => "physical frame allocation failed during init launch",
            Self::Map(arch::x86_64::MapError::AddressOverflow) => {
                "init executable mapping overflowed the address space"
            }
            Self::Map(arch::x86_64::MapError::PageTableAllocationFailed) => {
                "init executable mapping could not allocate page tables"
            }
            Self::Map(arch::x86_64::MapError::MappingConflict) => {
                "init executable launch hit an existing virtual mapping"
            }
        }
    }
}

pub fn activate_init_handoff() -> Result<InitExecSummary, InitExecActivateError> {
    let result = (|| {
        let image = vfs::materialize_init_image().map_err(InitExecActivateError::Load)?;
        build_activation(image)
    })();

    let state = state_mut();
    match result {
        Ok(activation) => {
            let summary = activation_summary(&activation);
            state.activation = Some(activation);
            state.last_error = None;
            state.last_launch_error = None;
            Ok(summary)
        }
        Err(error) => {
            state.activation = None;
            state.last_error = Some(error);
            state.last_launch_error = None;
            Err(error)
        }
    }
}

pub fn launch_exec_path(
    process_id: u64,
    exec_path: &str,
    stack: &syscall::BootstrapExecStackImage,
) -> Result<(), InitExecLaunchError> {
    let result = (|| {
        let image = vfs::materialize_executable_image(exec_path).map_err(InitExecLaunchError::Prep)?;
        match image.format {
            ExecutableFormat::Elf => {
                let interpreter_source = image.interpreter_source.clone();
                let activation = build_activation(image).map_err(InitExecLaunchError::Activation)?;
                if let Some(interpreter_path) = interpreter_source.as_deref() {
                    let interpreter_activation = materialize_launch_activation(interpreter_path)?;
                    launch_activation(
                        process_id,
                        exec_path,
                        Some(&activation),
                        &interpreter_activation,
                        stack,
                    )
                } else {
                    launch_activation(process_id, exec_path, None, &activation, stack)
                }
            }
            ExecutableFormat::ShebangScript => {
                let interpreter_path = image
                    .interpreter_source
                    .as_deref()
                    .ok_or(InitExecLaunchError::MissingInterpreter)?;
                let interpreter_activation = materialize_launch_activation(interpreter_path)?;
                launch_activation(process_id, exec_path, None, &interpreter_activation, stack)
            }
            ExecutableFormat::Text | ExecutableFormat::Unknown => {
                Err(InitExecLaunchError::Activation(
                    InitExecActivateError::UnsupportedFormat,
                ))
            }
        }
    })();

    state_mut().last_launch_error = match result {
        Ok(()) => None,
        Err(error) => Some(error),
    };
    result
}

pub fn discard_staged_activation() {
    state_mut().activation = None;
}

pub fn observe_timer_tick(tick: u64) {
    let mut remaining = LAUNCH_MONITOR_BUDGET.load(Ordering::Acquire);
    while remaining != 0 {
        match LAUNCH_MONITOR_BUDGET.compare_exchange(
            remaining,
            remaining - 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                kprintln!(
                    "HXNU: init launch heartbeat tick={} remaining={}",
                    tick,
                    remaining - 1,
                );
                return;
            }
            Err(updated) => remaining = updated,
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
    let last_launch_error = state
        .last_launch_error
        .map(|error| error.as_str())
        .unwrap_or("<none>");
    let _ = writeln!(text, "last_launch_error {}", last_launch_error);
    text
}

fn build_activation(image: vfs::ExecutableLoadImage) -> Result<ActivatedInitImage, InitExecActivateError> {
    if image.format != ExecutableFormat::Elf {
        return Err(InitExecActivateError::UnsupportedFormat);
    }

    let entry_point = image.entry_point.ok_or(InitExecActivateError::MissingEntryPoint)?;
    let image_type = image.image_type.ok_or(InitExecActivateError::MissingImageType)?;
    let machine = image.machine.ok_or(InitExecActivateError::MissingMachine)?;
    if image.vm_map_images.is_empty() {
        return Err(InitExecActivateError::NoLoadSegments);
    }

    let mut vm_start = u64::MAX;
    let mut vm_end = 0u64;
    let mut has_executable_segment = false;
    let mut entry_segment_index = None;
    let mut entry_segment_map_offset = 0u64;
    let mut segments = Vec::with_capacity(image.vm_map_images.len());

    for segment in image.vm_map_images.into_iter() {
        let expected_len = segment
            .map_end
            .checked_sub(segment.map_start)
            .ok_or(InitExecActivateError::InvalidSegmentMap)?;
        let actual_len = u64::try_from(segment.bytes.len()).map_err(|_| InitExecActivateError::InvalidSegmentMap)?;
        if expected_len != actual_len {
            return Err(InitExecActivateError::InvalidSegmentMap);
        }

        vm_start = vm_start.min(segment.map_start);
        vm_end = vm_end.max(segment.map_end);

        if segment.executable {
            has_executable_segment = true;
            if entry_point >= segment.virtual_start && entry_point < segment.virtual_end {
                entry_segment_index = Some(segment.index);
                entry_segment_map_offset = entry_point
                    .checked_sub(segment.map_start)
                    .ok_or(InitExecActivateError::InvalidSegmentMap)?;
            }
        }

        segments.push(to_activated_segment(segment));
    }

    if !has_executable_segment {
        return Err(InitExecActivateError::NoExecutableSegments);
    }
    let entry_segment_index = entry_segment_index.ok_or(InitExecActivateError::EntryOutsideExecutableSegments)?;

    Ok(ActivatedInitImage {
        path: image.path,
        format: image.format,
        image_type,
        machine,
        entry_point,
        vm_start,
        vm_end,
        total_bytes: image.vm_map_total_bytes,
        zero_fill_bytes: image.vm_map_zero_fill_bytes,
        entry_segment_index,
        entry_segment_map_offset,
        segments,
    })
}

fn to_activated_segment(segment: VmMapImageEntry) -> ActivatedSegment {
    ActivatedSegment {
        index: segment.index,
        virtual_start: segment.virtual_start,
        virtual_end: segment.virtual_end,
        map_start: segment.map_start,
        map_end: segment.map_end,
        file_bytes: segment.file_bytes,
        memory_bytes: segment.memory_bytes,
        readable: segment.readable,
        writable: segment.writable,
        executable: segment.executable,
        bytes: segment.bytes,
    }
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

fn launch_activation(
    process_id: u64,
    exec_path: &str,
    program_activation: Option<&ActivatedInitImage>,
    launch_activation: &ActivatedInitImage,
    stack: &syscall::BootstrapExecStackImage,
) -> Result<(), InitExecLaunchError> {
    if launch_activation.machine != ELF_MACHINE_X86_64 {
        return Err(InitExecLaunchError::UnsupportedMachine);
    }
    if program_activation.is_some_and(|activation| activation.machine != ELF_MACHINE_X86_64) {
        return Err(InitExecLaunchError::UnsupportedMachine);
    }

    let hhdm_offset = limine::hhdm_offset().ok_or(InitExecLaunchError::MissingHhdm)?;
    let launch_entry_point = stack
        .launch_entry_point
        .or(stack.entry_point)
        .unwrap_or(launch_activation.entry_point);
    let mut previous_launch = take_active_launch(process_id);
    let mut newly_mapped_pages = Vec::new();

    if let Some(previous) = previous_launch.as_ref() {
        teardown_launch(previous, hhdm_offset)?;
    }
    if let Err(error) = map_stack_image(&stack, hhdm_offset, &mut newly_mapped_pages) {
        kprintln!("HXNU: exec replace stack-map failed reason={}", error.as_str());
        rollback_launch(previous_launch.as_ref(), &newly_mapped_pages, hhdm_offset);
        return Err(error);
    }
    if let Some(activation) = program_activation {
        for segment in &activation.segments {
            if let Err(error) = map_loaded_segment(segment, hhdm_offset, &mut newly_mapped_pages) {
                kprintln!(
                    "HXNU: exec replace segment-map failed path={} idx={} reason={}",
                    activation.path,
                    segment.index,
                    error.as_str(),
                );
                rollback_launch(previous_launch.as_ref(), &newly_mapped_pages, hhdm_offset);
                return Err(error);
            }
        }
    }
    for segment in &launch_activation.segments {
        if let Err(error) = map_loaded_segment(segment, hhdm_offset, &mut newly_mapped_pages) {
            kprintln!(
                "HXNU: exec replace segment-map failed path={} idx={} reason={}",
                launch_activation.path,
                segment.index,
                error.as_str(),
            );
            rollback_launch(previous_launch.as_ref(), &newly_mapped_pages, hhdm_offset);
            return Err(error);
        }
    }
    record_active_launch(ActiveLaunch {
        process_id,
        path: String::from(exec_path),
        pages: newly_mapped_pages,
    });
    let exec_commit = syscall::commit_bootstrap_exec(exec_path);

    kprintln!(
        "HXNU: init exec-commit pid={} comm={} cloexec-closed={} mappings-cleared={} brk-reset={} clear-tid-cleared={} sigactions-cleared={} tls-reset={} robust-list-cleared={} rseq-cleared={}",
        exec_commit.process_id,
        exec_commit.comm_name,
        exec_commit.cloexec_closed,
        exec_commit.mappings_cleared,
        exec_commit.brk_reset,
        exec_commit.clear_tid_cleared,
        exec_commit.signal_actions_cleared,
        exec_commit.arch_prctl_reset,
        exec_commit.robust_list_cleared,
        exec_commit.rseq_cleared,
    );
    if let Some(previous) = previous_launch.take() {
        kprintln!(
            "HXNU: exec replace old-path={} old-pages={} new-path={} new-pages={}",
            previous.path,
            previous.pages.len(),
            exec_path,
            active_launch_page_count(),
        );
    }
    kprintln!(
        "HXNU: init launch transfer path={} launch={} entry={:#018x} stack={:#018x} bytes={} argv={} env={} auxv={} sig={:#018x}",
        exec_path,
        launch_activation.path,
        launch_entry_point,
        stack.stack_pointer,
        stack.stack_bytes,
        stack.argv_count,
        stack.env_count,
        stack.auxv_count,
        stack.signature,
    );
    LAUNCH_MONITOR_BUDGET.store(INIT_LAUNCH_MONITOR_TICKS, Ordering::Release);
    unsafe { jump_to_loaded_image(launch_entry_point, stack.stack_pointer) }
}

fn materialize_launch_activation(path: &str) -> Result<ActivatedInitImage, InitExecLaunchError> {
    let image = vfs::materialize_executable_image(path).map_err(InitExecLaunchError::Prep)?;
    build_activation(image).map_err(InitExecLaunchError::Activation)
}

fn map_stack_image(
    stack: &syscall::BootstrapExecStackImage,
    hhdm_offset: u64,
    tracked_pages: &mut Vec<TrackedPage>,
) -> Result<(), InitExecLaunchError> {
    let stack_end = stack
        .stack_pointer
        .checked_add(stack.stack_bytes as u64)
        .ok_or(InitExecLaunchError::StackRangeInvalid)?;
    if stack.stack_bytes != stack.bytes.len() || stack_end != stack.stack_top {
        return Err(InitExecLaunchError::StackRangeInvalid);
    }
    let image_page_start = align_down_to_page(stack.stack_pointer);
    let reserve_start = align_down_to_page(stack.stack_pointer.saturating_sub(BOOTSTRAP_STACK_RESERVE_BYTES));
    if reserve_start < image_page_start {
        map_zero_pages(reserve_start, image_page_start, hhdm_offset, tracked_pages)?;
    }
    map_image_bytes(stack.stack_pointer, &stack.bytes, hhdm_offset, tracked_pages)
}

fn map_loaded_segment(
    segment: &ActivatedSegment,
    hhdm_offset: u64,
    tracked_pages: &mut Vec<TrackedPage>,
) -> Result<(), InitExecLaunchError> {
    let mapped_len = segment
        .map_end
        .checked_sub(segment.map_start)
        .ok_or(InitExecLaunchError::SegmentRangeInvalid)?;
    let actual_len =
        u64::try_from(segment.bytes.len()).map_err(|_| InitExecLaunchError::SegmentRangeInvalid)?;
    if mapped_len != actual_len {
        return Err(InitExecLaunchError::SegmentRangeInvalid);
    }
    map_image_bytes(segment.map_start, &segment.bytes, hhdm_offset, tracked_pages)
}

fn map_image_bytes(
    virtual_start: u64,
    bytes: &[u8],
    hhdm_offset: u64,
    tracked_pages: &mut Vec<TrackedPage>,
) -> Result<(), InitExecLaunchError> {
    if bytes.is_empty() {
        return Ok(());
    }

    let image_end = virtual_start
        .checked_add(bytes.len() as u64)
        .ok_or(InitExecLaunchError::SegmentRangeInvalid)?;
    let mut source_offset = 0usize;
    let first_page = virtual_start & !PAGE_MASK;
    let mut page_address = first_page;
    let end_page = image_end.saturating_sub(1) & !PAGE_MASK;

    loop {
        let frame = mm::frame::allocate_frame().ok_or(InitExecLaunchError::FrameAllocationFailed)?;
        arch::x86_64::map_virtual_page(hhdm_offset, page_address, frame.start_address(), arch::x86_64::PAGE_USER)
            .map_err(InitExecLaunchError::Map)?;
        tracked_pages.push(TrackedPage {
            virtual_address: page_address,
            physical_address: frame.start_address(),
        });

        let frame_ptr = hhdm_offset
            .checked_add(frame.start_address())
            .ok_or(InitExecLaunchError::Map(arch::x86_64::MapError::AddressOverflow))?
            as *mut u8;
        let page_offset = if page_address == first_page {
            usize::try_from(virtual_start.saturating_sub(page_address))
                .map_err(|_| InitExecLaunchError::SegmentRangeInvalid)?
        } else {
            0
        };
        let copy_len = PAGE_BYTES
            .saturating_sub(page_offset)
            .min(bytes.len().saturating_sub(source_offset));

        unsafe {
            ptr::write_bytes(frame_ptr, 0, PAGE_BYTES);
            ptr::copy_nonoverlapping(bytes.as_ptr().add(source_offset), frame_ptr.add(page_offset), copy_len);
        }

        source_offset = source_offset.saturating_add(copy_len);
        if page_address == end_page {
            break;
        }

        page_address = page_address
            .checked_add(mm::frame::PAGE_SIZE)
            .ok_or(InitExecLaunchError::SegmentRangeInvalid)?;
    }

    Ok(())
}

fn map_zero_pages(
    start: u64,
    end: u64,
    hhdm_offset: u64,
    tracked_pages: &mut Vec<TrackedPage>,
) -> Result<(), InitExecLaunchError> {
    let mut page_address = align_down_to_page(start);
    let page_end = align_down_to_page(end);
    while page_address < page_end {
        let frame = mm::frame::allocate_frame().ok_or(InitExecLaunchError::FrameAllocationFailed)?;
        arch::x86_64::map_virtual_page(hhdm_offset, page_address, frame.start_address(), arch::x86_64::PAGE_USER)
            .map_err(InitExecLaunchError::Map)?;
        tracked_pages.push(TrackedPage {
            virtual_address: page_address,
            physical_address: frame.start_address(),
        });

        let frame_ptr = hhdm_offset
            .checked_add(frame.start_address())
            .ok_or(InitExecLaunchError::Map(arch::x86_64::MapError::AddressOverflow))?
            as *mut u8;
        unsafe {
            ptr::write_bytes(frame_ptr, 0, PAGE_BYTES);
        }

        page_address = page_address
            .checked_add(mm::frame::PAGE_SIZE)
            .ok_or(InitExecLaunchError::SegmentRangeInvalid)?;
    }
    Ok(())
}

const fn align_down_to_page(address: u64) -> u64 {
    address & !PAGE_MASK
}

fn take_active_launch(process_id: u64) -> Option<ActiveLaunch> {
    let state = state_mut();
    let launch = state.active_launch.take()?;
    if launch.process_id == process_id {
        return Some(launch);
    }
    state.active_launch = Some(launch);
    None
}

fn record_active_launch(launch: ActiveLaunch) {
    state_mut().active_launch = Some(launch);
}

fn active_launch_page_count() -> usize {
    state_ref()
        .active_launch
        .as_ref()
        .map_or(0, |launch| launch.pages.len())
}

fn teardown_launch(launch: &ActiveLaunch, hhdm_offset: u64) -> Result<(), InitExecLaunchError> {
    for page in &launch.pages {
        arch::x86_64::unmap_virtual_page(hhdm_offset, page.virtual_address)
            .map_err(InitExecLaunchError::Map)?;
    }
    Ok(())
}

fn restore_launch(launch: &ActiveLaunch, hhdm_offset: u64) -> Result<(), InitExecLaunchError> {
    for page in &launch.pages {
        arch::x86_64::map_virtual_page(
            hhdm_offset,
            page.virtual_address,
            page.physical_address,
            arch::x86_64::PAGE_USER,
        )
        .map_err(InitExecLaunchError::Map)?;
    }
    Ok(())
}

fn cleanup_partial_launch(
    mapped_pages: &[TrackedPage],
    hhdm_offset: u64,
) -> Result<(), InitExecLaunchError> {
    for page in mapped_pages {
        arch::x86_64::unmap_virtual_page(hhdm_offset, page.virtual_address)
            .map_err(InitExecLaunchError::Map)?;
    }
    Ok(())
}

fn rollback_launch(
    previous_launch: Option<&ActiveLaunch>,
    newly_mapped_pages: &[TrackedPage],
    hhdm_offset: u64,
) {
    let _ = cleanup_partial_launch(newly_mapped_pages, hhdm_offset);
    if let Some(previous) = previous_launch {
        let _ = restore_launch(previous, hhdm_offset);
        record_active_launch(previous.clone());
    }
}

unsafe fn jump_to_loaded_image(entry_point: u64, stack_pointer: u64) -> ! {
    unsafe {
        asm!(
            "mov rsp, {stack}",
            "xor rbp, rbp",
            "sti",
            "jmp {entry}",
            stack = in(reg) stack_pointer,
            entry = in(reg) entry_point,
            options(noreturn),
        );
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
