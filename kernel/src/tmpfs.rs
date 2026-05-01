use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::fmt::Write;

const TMP_ROOT_PATH: &str = "/tmp";
const RUN_ROOT_PATH: &str = "/run";
const MAX_TMPFS_FILES: usize = 256;

struct GlobalTmpfs(UnsafeCell<Option<TmpfsState>>);

unsafe impl Sync for GlobalTmpfs {}

impl GlobalTmpfs {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<TmpfsState> {
        self.0.get()
    }
}

static TMPFS: GlobalTmpfs = GlobalTmpfs::new();

struct TmpfsState {
    initialized: bool,
    next_file_id: u64,
    files: Vec<TmpfsFile>,
}

struct TmpfsFile {
    id: u64,
    path: Option<String>,
    open_count: usize,
    content: Vec<u8>,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum TmpfsNodeKind {
    Directory,
    File,
}

#[derive(Copy, Clone)]
pub struct TmpfsNodeInfo {
    pub size: usize,
}

#[derive(Copy, Clone)]
pub struct TmpfsSummary {
    pub directory_count: usize,
    pub file_count: usize,
    pub entry_count: usize,
    pub total_bytes: usize,
}

pub struct TmpfsOpenFile {
    pub file_id: u64,
    pub path: String,
    pub content: Vec<u8>,
}

pub struct TmpfsFileStatus {
    pub file_id: u64,
    pub size: usize,
    pub link_count: usize,
}

#[derive(Copy, Clone)]
pub enum TmpfsError {
    AlreadyInitialized,
    NotInitialized,
    InvalidPath,
    NotFound,
    IsDirectory,
    FileLimitReached,
}

impl TmpfsError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "tmpfs is already initialized",
            Self::NotInitialized => "tmpfs is not initialized",
            Self::InvalidPath => "tmpfs path is invalid",
            Self::NotFound => "tmpfs entry not found",
            Self::IsDirectory => "tmpfs path resolves to a directory",
            Self::FileLimitReached => "tmpfs file limit reached",
        }
    }
}

pub fn initialize() -> Result<TmpfsSummary, TmpfsError> {
    let slot = unsafe { &mut *TMPFS.get() };
    if slot.is_some() {
        return Err(TmpfsError::AlreadyInitialized);
    }

    *slot = Some(TmpfsState {
        initialized: true,
        next_file_id: 1,
        files: Vec::new(),
    });
    Ok(summary())
}

pub fn is_initialized() -> bool {
    unsafe { (&*TMPFS.get()).as_ref().is_some_and(|state| state.initialized) }
}

pub fn summary() -> TmpfsSummary {
    let Some(state) = (unsafe { (&*TMPFS.get()).as_ref() }) else {
        return TmpfsSummary {
            directory_count: 0,
            file_count: 0,
            entry_count: 0,
            total_bytes: 0,
        };
    };

    let total_bytes = state
        .files
        .iter()
        .filter(|file| file.path.is_some())
        .fold(0usize, |total, file| total.saturating_add(file.content.len()));
    let file_count = state.files.iter().filter(|file| file.path.is_some()).count();
    TmpfsSummary {
        directory_count: 2,
        file_count,
        entry_count: 2usize.saturating_add(file_count),
        total_bytes,
    }
}

pub fn handles_path(path: &str) -> bool {
    path == TMP_ROOT_PATH
        || path == RUN_ROOT_PATH
        || path.starts_with("/tmp/")
        || path.starts_with("/run/")
}

pub fn node_kind(path: &str) -> Option<TmpfsNodeKind> {
    let normalized = normalize_path(path)?;
    if normalized == TMP_ROOT_PATH || normalized == RUN_ROOT_PATH {
        return Some(TmpfsNodeKind::Directory);
    }

    let state = unsafe { (&*TMPFS.get()).as_ref()? };
    if state
        .files
        .iter()
        .any(|file| file.path.as_deref() == Some(normalized.as_str()))
    {
        return Some(TmpfsNodeKind::File);
    }
    None
}

pub fn node_info(path: &str) -> Option<TmpfsNodeInfo> {
    match node_kind(path)? {
        TmpfsNodeKind::Directory => Some(TmpfsNodeInfo { size: read(path)?.len() }),
        TmpfsNodeKind::File => {
            let normalized = normalize_path(path)?;
            let state = unsafe { (&*TMPFS.get()).as_ref()? };
            let file = state
                .files
                .iter()
                .find(|file| file.path.as_deref() == Some(normalized.as_str()))?;
            Some(TmpfsNodeInfo {
                size: file.content.len(),
            })
        }
    }
}

pub fn read(path: &str) -> Option<String> {
    let normalized = normalize_path(path)?;
    match node_kind(&normalized)? {
        TmpfsNodeKind::Directory => render_directory(&normalized),
        TmpfsNodeKind::File => {
            let bytes = read_bytes(&normalized)?;
            Some(String::from_utf8_lossy(&bytes).into_owned())
        }
    }
}

pub fn read_bytes(path: &str) -> Option<Vec<u8>> {
    let normalized = normalize_path(path)?;
    let state = unsafe { (&*TMPFS.get()).as_ref()? };
    let file = state
        .files
        .iter()
        .find(|file| file.path.as_deref() == Some(normalized.as_str()))?;
    Some(file.content.clone())
}

pub fn open_file(path: &str, create: bool, truncate: bool) -> Result<TmpfsOpenFile, TmpfsError> {
    let normalized = normalize_path(path).ok_or(TmpfsError::InvalidPath)?;
    ensure_tmpfs_parent(&normalized)?;
    let state = unsafe { (&mut *TMPFS.get()).as_mut() }.ok_or(TmpfsError::NotInitialized)?;

    if normalized == TMP_ROOT_PATH || normalized == RUN_ROOT_PATH {
        return Err(TmpfsError::IsDirectory);
    }

    if let Some(file) = state
        .files
        .iter_mut()
        .find(|file| file.path.as_deref() == Some(normalized.as_str()))
    {
        if truncate {
            file.content.clear();
        }
        file.open_count = file.open_count.saturating_add(1);
        return Ok(TmpfsOpenFile {
            file_id: file.id,
            path: normalized,
            content: file.content.clone(),
        });
    }

    if !create {
        return Err(TmpfsError::NotFound);
    }
    if state.files.len() >= MAX_TMPFS_FILES {
        return Err(TmpfsError::FileLimitReached);
    }

    let file_id = state.next_file_id;
    state.next_file_id = state.next_file_id.saturating_add(1);
    state.files.push(TmpfsFile {
        id: file_id,
        path: Some(normalized.clone()),
        open_count: 1,
        content: Vec::new(),
    });
    Ok(TmpfsOpenFile {
        file_id,
        path: normalized,
        content: Vec::new(),
    })
}

pub fn write_file(path: &str, content: &[u8]) -> Result<(), TmpfsError> {
    let normalized = normalize_path(path).ok_or(TmpfsError::InvalidPath)?;
    ensure_tmpfs_parent(&normalized)?;
    let state = unsafe { (&mut *TMPFS.get()).as_mut() }.ok_or(TmpfsError::NotInitialized)?;

    let Some(file) = state
        .files
        .iter_mut()
        .find(|file| file.path.as_deref() == Some(normalized.as_str()))
    else {
        return Err(TmpfsError::NotFound);
    };
    file.content.clear();
    file.content.extend_from_slice(content);
    Ok(())
}

pub fn read_bytes_by_id(file_id: u64) -> Option<Vec<u8>> {
    let state = unsafe { (&*TMPFS.get()).as_ref()? };
    let file = state.files.iter().find(|file| file.id == file_id)?;
    Some(file.content.clone())
}

pub fn write_file_by_id(file_id: u64, content: &[u8]) -> Result<(), TmpfsError> {
    let state = unsafe { (&mut *TMPFS.get()).as_mut() }.ok_or(TmpfsError::NotInitialized)?;
    let Some(file) = state.files.iter_mut().find(|file| file.id == file_id) else {
        return Err(TmpfsError::NotFound);
    };
    file.content.clear();
    file.content.extend_from_slice(content);
    Ok(())
}

pub fn increment_open_count(file_id: u64) -> Result<(), TmpfsError> {
    let state = unsafe { (&mut *TMPFS.get()).as_mut() }.ok_or(TmpfsError::NotInitialized)?;
    let Some(file) = state.files.iter_mut().find(|file| file.id == file_id) else {
        return Err(TmpfsError::NotFound);
    };
    file.open_count = file.open_count.saturating_add(1);
    Ok(())
}

pub fn close_file_handle(file_id: u64) -> Result<(), TmpfsError> {
    let state = unsafe { (&mut *TMPFS.get()).as_mut() }.ok_or(TmpfsError::NotInitialized)?;
    let Some(index) = state.files.iter().position(|file| file.id == file_id) else {
        return Err(TmpfsError::NotFound);
    };
    if state.files[index].open_count != 0 {
        state.files[index].open_count -= 1;
    }
    collect_orphaned_file(state, index);
    Ok(())
}

pub fn file_status_by_id(file_id: u64) -> Option<TmpfsFileStatus> {
    let state = unsafe { (&*TMPFS.get()).as_ref()? };
    let file = state.files.iter().find(|file| file.id == file_id)?;
    Some(TmpfsFileStatus {
        file_id: file.id,
        size: file.content.len(),
        link_count: usize::from(file.path.is_some()),
    })
}

pub fn unlink_file(path: &str) -> Result<(), TmpfsError> {
    let normalized = normalize_path(path).ok_or(TmpfsError::InvalidPath)?;
    if normalized == TMP_ROOT_PATH || normalized == RUN_ROOT_PATH {
        return Err(TmpfsError::IsDirectory);
    }

    let state = unsafe { (&mut *TMPFS.get()).as_mut() }.ok_or(TmpfsError::NotInitialized)?;
    let Some(index) = state
        .files
        .iter()
        .position(|file| file.path.as_deref() == Some(normalized.as_str()))
    else {
        return Err(TmpfsError::NotFound);
    };
    state.files[index].path = None;
    collect_orphaned_file(state, index);
    Ok(())
}

pub fn rename_file(source_path: &str, destination_path: &str) -> Result<(), TmpfsError> {
    let source = normalize_path(source_path).ok_or(TmpfsError::InvalidPath)?;
    let destination = normalize_path(destination_path).ok_or(TmpfsError::InvalidPath)?;
    ensure_tmpfs_parent(&source)?;
    ensure_tmpfs_parent(&destination)?;
    if source == TMP_ROOT_PATH
        || source == RUN_ROOT_PATH
        || destination == TMP_ROOT_PATH
        || destination == RUN_ROOT_PATH
    {
        return Err(TmpfsError::IsDirectory);
    }
    if source == destination {
        return Ok(());
    }

    let state = unsafe { (&mut *TMPFS.get()).as_mut() }.ok_or(TmpfsError::NotInitialized)?;
    let Some(mut source_index) = state
        .files
        .iter()
        .position(|file| file.path.as_deref() == Some(source.as_str()))
    else {
        return Err(TmpfsError::NotFound);
    };

    if let Some(destination_index) = state
        .files
        .iter()
        .position(|file| file.path.as_deref() == Some(destination.as_str()))
    {
        state.files[destination_index].path = None;
        collect_orphaned_file(state, destination_index);
        if destination_index < source_index {
            source_index -= 1;
        }
    }

    state.files[source_index].path = Some(destination);
    Ok(())
}

fn ensure_tmpfs_parent(path: &str) -> Result<(), TmpfsError> {
    if !handles_path(path) {
        return Err(TmpfsError::InvalidPath);
    }

    if path == TMP_ROOT_PATH || path == RUN_ROOT_PATH {
        return Ok(());
    }

    let parent = parent_directory(path).ok_or(TmpfsError::InvalidPath)?;
    if parent == TMP_ROOT_PATH || parent == RUN_ROOT_PATH {
        return Ok(());
    }

    Err(TmpfsError::InvalidPath)
}

fn parent_directory(path: &str) -> Option<&str> {
    let slash = path.rfind('/')?;
    if slash == 0 {
        return Some("/");
    }
    path.get(..slash)
}

fn render_directory(path: &str) -> Option<String> {
    let state = unsafe { (&*TMPFS.get()).as_ref()? };
    let mut entries: Vec<String> = Vec::new();
    for file in &state.files {
        let Some(file_path) = file.path.as_deref() else {
            continue;
        };
        if let Some(name) = direct_child_name(path, file_path) {
            entries.push(String::from(name));
        }
    }
    entries.sort_unstable();

    let mut text = String::new();
    for entry in entries {
        let _ = writeln!(text, "{}", entry);
    }
    Some(text)
}

fn direct_child_name<'a>(parent: &str, path: &'a str) -> Option<&'a str> {
    if !path.starts_with(parent) {
        return None;
    }

    let parent_len = parent.len();
    let suffix = path.get(parent_len..)?;
    let suffix = suffix.strip_prefix('/')?;
    if suffix.is_empty() || suffix.contains('/') {
        return None;
    }

    Some(suffix)
}

fn normalize_path(path: &str) -> Option<String> {
    if !path.starts_with('/') {
        return None;
    }

    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            segments.pop()?;
            continue;
        }
        segments.push(segment);
    }

    if segments.is_empty() {
        return Some(String::from("/"));
    }

    let mut normalized = String::from("/");
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            normalized.push('/');
        }
        normalized.push_str(segment);
    }
    Some(normalized)
}

fn collect_orphaned_file(state: &mut TmpfsState, index: usize) {
    if index >= state.files.len() {
        return;
    }
    if state.files[index].path.is_none() && state.files[index].open_count == 0 {
        state.files.remove(index);
    }
}
