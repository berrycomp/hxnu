#![allow(dead_code)]

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::char;
use core::fmt::Write;

use crate::block;

const FAT_PATH_ROOT: &str = "/fat";
const DIRECTORY_ENTRY_BYTES: usize = 32;
const FAT16_EOC_MIN: u32 = 0x0000_fff8;
const FAT16_BAD_CLUSTER: u32 = 0x0000_fff7;
const FAT32_EOC_MIN: u32 = 0x0fff_fff8;
const FAT32_BAD_CLUSTER: u32 = 0x0fff_fff7;
const MAX_CLUSTER_CHAIN_STEPS: usize = 4096;
const MAX_DIRECTORY_RECURSION_DEPTH: usize = 16;
const LFN_ATTRIBUTE: u8 = 0x0f;
const ATTR_VOLUME_ID: u8 = 0x08;
const ATTR_DIRECTORY: u8 = 0x10;

struct GlobalFat(UnsafeCell<Option<FatState>>);

unsafe impl Sync for GlobalFat {}

impl GlobalFat {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<FatState> {
        self.0.get()
    }
}

static FAT: GlobalFat = GlobalFat::new();

struct FatState {
    summary: FatSummary,
    nodes: Vec<FatNode>,
}

struct FatNode {
    path: String,
    name: String,
    kind: FatNodeKind,
    size: usize,
    content: Vec<u8>,
    children: Vec<usize>,
}

#[derive(Clone)]
struct FatDirectoryEntry {
    name: String,
    kind: FatNodeKind,
    size: usize,
    first_cluster: u32,
}

struct PendingLongName {
    fragments: Vec<String>,
}

impl PendingLongName {
    fn new() -> Self {
        Self {
            fragments: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.fragments.clear();
    }

    fn push_fragment(&mut self, fragment: String, reset: bool) {
        if reset {
            self.fragments.clear();
        }
        self.fragments.push(fragment);
    }

    fn take_or(&mut self, fallback: String) -> String {
        if self.fragments.is_empty() {
            return fallback;
        }

        let mut text = String::new();
        for fragment in self.fragments.iter().rev() {
            text.push_str(fragment);
        }
        self.fragments.clear();
        if text.is_empty() { fallback } else { text }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum FatType {
    Fat16,
    Fat32,
}

impl FatType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fat16 => "fat16",
            Self::Fat32 => "fat32",
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum FatNodeKind {
    Directory,
    File,
}

#[derive(Copy, Clone)]
pub struct FatNodeInfo {
    pub kind: FatNodeKind,
    pub size: usize,
}

#[derive(Copy, Clone)]
pub struct FatSummary {
    pub mounted: bool,
    pub partition_id: Option<u16>,
    pub device_id: Option<u16>,
    pub partition_table: Option<block::PartitionTableKind>,
    pub fat_type: Option<FatType>,
    pub root_entry_count: usize,
    pub directory_count: usize,
    pub file_count: usize,
}

impl FatSummary {
    const fn offline() -> Self {
        Self {
            mounted: false,
            partition_id: None,
            device_id: None,
            partition_table: None,
            fat_type: None,
            root_entry_count: 0,
            directory_count: 0,
            file_count: 0,
        }
    }
}

#[derive(Copy, Clone)]
pub enum FatError {
    AlreadyInitialized,
    BlockUnavailable,
    NoFatPartition,
}

impl FatError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "fat is already initialized",
            Self::BlockUnavailable => "block layer is unavailable",
            Self::NoFatPartition => "no FAT16/32 partition found",
        }
    }
}

pub fn initialize() -> Result<FatSummary, FatError> {
    let slot = unsafe { &mut *FAT.get() };
    if slot.is_some() {
        return Err(FatError::AlreadyInitialized);
    }
    if !block::is_initialized() {
        return Err(FatError::BlockUnavailable);
    }

    let mut index = 0usize;
    while index < block::partition_count() {
        if let Some(partition) = block::partition(index) {
            if let Some(state) = try_mount_partition(partition) {
                let summary = state.summary;
                *slot = Some(state);
                return Ok(summary);
            }
        }
        index += 1;
    }

    Err(FatError::NoFatPartition)
}

pub fn is_initialized() -> bool {
    unsafe { (&*FAT.get()).is_some() }
}

pub fn summary() -> FatSummary {
    let Some(state) = (unsafe { (&*FAT.get()).as_ref() }) else {
        return FatSummary::offline();
    };
    state.summary
}

pub fn node_kind(path: &str) -> Option<FatNodeKind> {
    let state = unsafe { (&*FAT.get()).as_ref()? };
    let node = find_node(state, path)?;
    Some(node.kind)
}

pub fn node_info(path: &str) -> Option<FatNodeInfo> {
    let state = unsafe { (&*FAT.get()).as_ref()? };
    let node = find_node(state, path)?;
    Some(FatNodeInfo {
        kind: node.kind,
        size: node.size,
    })
}

pub fn read(path: &str) -> Option<String> {
    let state = unsafe { (&*FAT.get()).as_ref()? };
    let node = find_node(state, path)?;
    match node.kind {
        FatNodeKind::Directory => Some(render_directory_listing(state.nodes.as_slice(), node)),
        FatNodeKind::File => Some(String::from_utf8_lossy(&node.content).into_owned()),
    }
}

pub fn first_root_file_path() -> Option<String> {
    let state = unsafe { (&*FAT.get()).as_ref()? };
    state
        .nodes
        .iter()
        .find(|node| node.kind == FatNodeKind::File && fat_path_depth(&node.path) == 1)
        .map(|node| node.path.clone())
}

pub fn first_nested_file_path() -> Option<String> {
    let state = unsafe { (&*FAT.get()).as_ref()? };
    state
        .nodes
        .iter()
        .find(|node| node.kind == FatNodeKind::File && fat_path_depth(&node.path) >= 2)
        .map(|node| node.path.clone())
}

fn try_mount_partition(partition: block::PartitionInfo) -> Option<FatState> {
    let mut bpb_sector = [0u8; block::SECTOR_BYTES];
    if block::read(partition.device_id, partition.start_lba, 1, &mut bpb_sector).is_err() {
        return None;
    }
    if bpb_sector[510] != 0x55 || bpb_sector[511] != 0xAA {
        return None;
    }

    let bpb = parse_bpb(&bpb_sector)?;
    let root_entries = read_root_directory_entries(partition, &bpb)?;
    let root_entry_count = root_entries.len();
    let mut nodes = vec![FatNode {
        path: String::from(FAT_PATH_ROOT),
        name: String::from("fat"),
        kind: FatNodeKind::Directory,
        size: 0,
        content: Vec::new(),
        children: Vec::new(),
    }];
    populate_directory_nodes(partition, &bpb, &mut nodes, 0, root_entries, 0)?;

    let directory_count = nodes
        .iter()
        .filter(|node| node.kind == FatNodeKind::Directory)
        .count();
    let file_count = nodes
        .iter()
        .filter(|node| node.kind == FatNodeKind::File)
        .count();

    Some(FatState {
        summary: FatSummary {
            mounted: true,
            partition_id: Some(partition.id),
            device_id: Some(partition.device_id),
            partition_table: Some(partition.table_kind),
            fat_type: Some(bpb.fat_type),
            root_entry_count,
            directory_count,
            file_count,
        },
        nodes,
    })
}

#[derive(Copy, Clone)]
struct BpbLayout {
    fat_type: FatType,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fat_count: u8,
    sectors_per_fat: u32,
    root_dir_entries: u16,
    root_dir_sectors: u32,
    fat_start_lba_offset: u32,
    root_dir_start_lba_offset: u32,
    first_data_sector_offset: u32,
    root_dir_first_cluster: u32,
}

fn parse_bpb(sector: &[u8; block::SECTOR_BYTES]) -> Option<BpbLayout> {
    let bytes_per_sector = read_u16_le(sector, 11);
    if bytes_per_sector != block::SECTOR_BYTES as u16 {
        return None;
    }

    let sectors_per_cluster = sector[13];
    if sectors_per_cluster == 0 {
        return None;
    }

    let reserved_sectors = read_u16_le(sector, 14);
    if reserved_sectors == 0 {
        return None;
    }

    let fat_count = sector[16];
    if fat_count == 0 {
        return None;
    }

    let root_dir_entries = read_u16_le(sector, 17);
    let total_sectors_16 = read_u16_le(sector, 19) as u32;
    let sectors_per_fat_16 = read_u16_le(sector, 22) as u32;
    let total_sectors_32 = read_u32_le(sector, 32);
    let sectors_per_fat_32 = read_u32_le(sector, 36);
    let root_dir_first_cluster = read_u32_le(sector, 44);

    let total_sectors = if total_sectors_16 != 0 {
        total_sectors_16
    } else {
        total_sectors_32
    };
    if total_sectors == 0 {
        return None;
    }

    let sectors_per_fat = if sectors_per_fat_16 != 0 {
        sectors_per_fat_16
    } else {
        sectors_per_fat_32
    };
    if sectors_per_fat == 0 {
        return None;
    }

    let root_dir_sectors =
        ((u32::from(root_dir_entries) * 32) + (u32::from(bytes_per_sector) - 1)) / u32::from(bytes_per_sector);
    let first_data_sector =
        u32::from(reserved_sectors) + (u32::from(fat_count) * sectors_per_fat) + root_dir_sectors;
    if first_data_sector >= total_sectors {
        return None;
    }

    let data_sectors = total_sectors.saturating_sub(first_data_sector);
    let cluster_count = data_sectors / u32::from(sectors_per_cluster);
    let fat_type = if cluster_count < 4_085 {
        return None;
    } else if cluster_count < 65_525 {
        FatType::Fat16
    } else {
        FatType::Fat32
    };

    if fat_type == FatType::Fat32 && root_dir_first_cluster < 2 {
        return None;
    }

    Some(BpbLayout {
        fat_type,
        bytes_per_sector,
        sectors_per_cluster,
        reserved_sectors,
        fat_count,
        sectors_per_fat,
        root_dir_entries,
        root_dir_sectors,
        fat_start_lba_offset: u32::from(reserved_sectors),
        root_dir_start_lba_offset: u32::from(reserved_sectors) + (u32::from(fat_count) * sectors_per_fat),
        first_data_sector_offset: first_data_sector,
        root_dir_first_cluster,
    })
}

fn populate_directory_nodes(
    partition: block::PartitionInfo,
    bpb: &BpbLayout,
    nodes: &mut Vec<FatNode>,
    parent_index: usize,
    entries: Vec<FatDirectoryEntry>,
    depth: usize,
) -> Option<()> {
    for entry in entries {
        let parent_path = nodes[parent_index].path.clone();
        let child_path = join_fat_path(&parent_path, &entry.name);
        let child_index = nodes.len();
        match entry.kind {
            FatNodeKind::Directory => {
                nodes.push(FatNode {
                    path: child_path.clone(),
                    name: entry.name.clone(),
                    kind: FatNodeKind::Directory,
                    size: 0,
                    content: Vec::new(),
                    children: Vec::new(),
                });
                nodes[parent_index].children.push(child_index);
                if depth < MAX_DIRECTORY_RECURSION_DEPTH {
                    let children = read_subdirectory_entries(partition, bpb, entry.first_cluster)?;
                    populate_directory_nodes(partition, bpb, nodes, child_index, children, depth + 1)?;
                }
                nodes[child_index].size = render_directory_listing(nodes.as_slice(), &nodes[child_index]).len();
            }
            FatNodeKind::File => {
                let content = read_file_content(partition, bpb, entry.first_cluster, entry.size)?;
                nodes.push(FatNode {
                    path: child_path,
                    name: entry.name,
                    kind: FatNodeKind::File,
                    size: entry.size,
                    content,
                    children: Vec::new(),
                });
                nodes[parent_index].children.push(child_index);
            }
        }
    }

    nodes[parent_index].size = render_directory_listing(nodes.as_slice(), &nodes[parent_index]).len();
    Some(())
}

fn read_root_directory_entries(partition: block::PartitionInfo, bpb: &BpbLayout) -> Option<Vec<FatDirectoryEntry>> {
    let bytes = match bpb.fat_type {
        FatType::Fat16 => read_fat16_root_directory_bytes(partition, bpb)?,
        FatType::Fat32 => read_cluster_chain_bytes(partition, bpb, bpb.root_dir_first_cluster)?,
    };
    Some(parse_directory_entries(&bytes, bpb.fat_type))
}

fn read_subdirectory_entries(
    partition: block::PartitionInfo,
    bpb: &BpbLayout,
    first_cluster: u32,
) -> Option<Vec<FatDirectoryEntry>> {
    if first_cluster < 2 {
        return Some(Vec::new());
    }
    let bytes = read_cluster_chain_bytes(partition, bpb, first_cluster)?;
    Some(parse_directory_entries(&bytes, bpb.fat_type))
}

fn read_fat16_root_directory_bytes(partition: block::PartitionInfo, bpb: &BpbLayout) -> Option<Vec<u8>> {
    let sector_count = usize::try_from(bpb.root_dir_sectors).ok()?;
    let mut bytes = Vec::with_capacity(sector_count.checked_mul(block::SECTOR_BYTES)?);
    let start_lba = partition.start_lba + u64::from(bpb.root_dir_start_lba_offset);
    let mut sector = [0u8; block::SECTOR_BYTES];

    let mut sector_offset = 0u32;
    while sector_offset < bpb.root_dir_sectors {
        if block::read(
            partition.device_id,
            start_lba + u64::from(sector_offset),
            1,
            &mut sector,
        )
        .is_err()
        {
            return None;
        }
        bytes.extend_from_slice(&sector);
        sector_offset += 1;
    }

    Some(bytes)
}

fn read_cluster_chain_bytes(
    partition: block::PartitionInfo,
    bpb: &BpbLayout,
    first_cluster: u32,
) -> Option<Vec<u8>> {
    if first_cluster < 2 {
        return Some(Vec::new());
    }

    let cluster_bytes = usize::from(bpb.sectors_per_cluster).checked_mul(block::SECTOR_BYTES)?;
    let mut bytes = Vec::new();
    let mut cluster = first_cluster;
    let mut steps = 0usize;
    let mut sector = [0u8; block::SECTOR_BYTES];

    while steps < MAX_CLUSTER_CHAIN_STEPS {
        let cluster_lba = cluster_to_lba(partition, bpb, cluster)?;
        let new_len = bytes.len().checked_add(cluster_bytes)?;
        bytes.reserve(new_len.saturating_sub(bytes.len()));

        let mut sector_index = 0u8;
        while sector_index < bpb.sectors_per_cluster {
            if block::read(partition.device_id, cluster_lba + u64::from(sector_index), 1, &mut sector).is_err() {
                return None;
            }
            bytes.extend_from_slice(&sector);
            sector_index += 1;
        }

        let next = read_fat_entry(partition, bpb, cluster)?;
        if is_bad_cluster(bpb.fat_type, next) {
            return None;
        }
        if next == 0 || next == cluster || is_end_of_chain(bpb.fat_type, next) {
            break;
        }

        cluster = next;
        steps += 1;
    }

    Some(bytes)
}

fn read_file_content(
    partition: block::PartitionInfo,
    bpb: &BpbLayout,
    first_cluster: u32,
    size: usize,
) -> Option<Vec<u8>> {
    if size == 0 {
        return Some(Vec::new());
    }
    if first_cluster < 2 {
        return None;
    }

    let mut bytes = read_cluster_chain_bytes(partition, bpb, first_cluster)?;
    if bytes.len() < size {
        return None;
    }
    bytes.truncate(size);
    Some(bytes)
}

fn cluster_to_lba(partition: block::PartitionInfo, bpb: &BpbLayout, cluster: u32) -> Option<u64> {
    if cluster < 2 {
        return None;
    }
    partition
        .start_lba
        .checked_add(u64::from(bpb.first_data_sector_offset))?
        .checked_add(u64::from(cluster.saturating_sub(2)) * u64::from(bpb.sectors_per_cluster))
}

fn read_fat_entry(partition: block::PartitionInfo, bpb: &BpbLayout, cluster: u32) -> Option<u32> {
    match bpb.fat_type {
        FatType::Fat16 => read_fat16_entry(partition, bpb, cluster),
        FatType::Fat32 => read_fat32_entry(partition, bpb, cluster),
    }
}

fn read_fat16_entry(partition: block::PartitionInfo, bpb: &BpbLayout, cluster: u32) -> Option<u32> {
    let fat_offset = u64::from(cluster) * 2;
    let fat_sector_lba = partition.start_lba + u64::from(bpb.fat_start_lba_offset) + (fat_offset / 512);
    let fat_sector_offset = (fat_offset % 512) as usize;
    let mut sector = [0u8; block::SECTOR_BYTES];
    if block::read(partition.device_id, fat_sector_lba, 1, &mut sector).is_err() {
        return None;
    }
    Some(u32::from(read_u16_le(&sector, fat_sector_offset)))
}

fn read_fat32_entry(partition: block::PartitionInfo, bpb: &BpbLayout, cluster: u32) -> Option<u32> {
    let fat_offset = u64::from(cluster) * 4;
    let fat_sector_lba = partition.start_lba + u64::from(bpb.fat_start_lba_offset) + (fat_offset / 512);
    let fat_sector_offset = (fat_offset % 512) as usize;
    let mut sector = [0u8; block::SECTOR_BYTES];
    if block::read(partition.device_id, fat_sector_lba, 1, &mut sector).is_err() {
        return None;
    }
    Some(read_u32_le(&sector, fat_sector_offset) & 0x0fff_ffff)
}

fn is_end_of_chain(fat_type: FatType, entry: u32) -> bool {
    match fat_type {
        FatType::Fat16 => entry >= FAT16_EOC_MIN,
        FatType::Fat32 => entry >= FAT32_EOC_MIN,
    }
}

fn is_bad_cluster(fat_type: FatType, entry: u32) -> bool {
    match fat_type {
        FatType::Fat16 => entry == FAT16_BAD_CLUSTER,
        FatType::Fat32 => entry == FAT32_BAD_CLUSTER,
    }
}

fn parse_directory_entries(bytes: &[u8], fat_type: FatType) -> Vec<FatDirectoryEntry> {
    let mut entries = Vec::new();
    let mut pending_long_name = PendingLongName::new();
    let mut offset = 0usize;

    while offset + DIRECTORY_ENTRY_BYTES <= bytes.len() {
        let entry = &bytes[offset..offset + DIRECTORY_ENTRY_BYTES];
        let first = entry[0];
        if first == 0x00 {
            break;
        }
        if first == 0xE5 {
            pending_long_name.clear();
            offset += DIRECTORY_ENTRY_BYTES;
            continue;
        }

        let attrs = entry[11];
        if attrs == LFN_ATTRIBUTE {
            let fragment = decode_lfn_fragment(entry);
            if let Some(fragment) = fragment {
                pending_long_name.push_fragment(fragment, (entry[0] & 0x40) != 0);
            } else {
                pending_long_name.clear();
            }
            offset += DIRECTORY_ENTRY_BYTES;
            continue;
        }
        if (attrs & ATTR_VOLUME_ID) != 0 {
            pending_long_name.clear();
            offset += DIRECTORY_ENTRY_BYTES;
            continue;
        }

        let Some(short_name) = parse_short_name(&entry[..11]) else {
            pending_long_name.clear();
            offset += DIRECTORY_ENTRY_BYTES;
            continue;
        };
        let name = pending_long_name.take_or(short_name);
        if name == "." || name == ".." {
            offset += DIRECTORY_ENTRY_BYTES;
            continue;
        }

        let kind = if (attrs & ATTR_DIRECTORY) != 0 {
            FatNodeKind::Directory
        } else {
            FatNodeKind::File
        };
        let size = read_u32_le(entry, 28) as usize;
        let cluster_low = u32::from(read_u16_le(entry, 26));
        let cluster_high = if fat_type == FatType::Fat32 {
            u32::from(read_u16_le(entry, 20))
        } else {
            0
        };
        entries.push(FatDirectoryEntry {
            name,
            kind,
            size,
            first_cluster: (cluster_high << 16) | cluster_low,
        });

        offset += DIRECTORY_ENTRY_BYTES;
    }

    entries
}

fn decode_lfn_fragment(entry: &[u8]) -> Option<String> {
    if entry.len() != DIRECTORY_ENTRY_BYTES {
        return None;
    }

    let mut text = String::new();
    for offset in [1usize, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30] {
        let codepoint = read_u16_le(entry, offset);
        if codepoint == 0x0000 || codepoint == 0xffff {
            break;
        }
        let ch = char::from_u32(u32::from(codepoint))?;
        text.push(ch);
    }
    Some(text)
}

fn parse_short_name(name: &[u8]) -> Option<String> {
    if name.len() != 11 {
        return None;
    }

    let base = parse_name_component(&name[..8])?;
    if base.is_empty() {
        return None;
    }
    let ext = parse_name_component(&name[8..])?;

    let mut full = String::new();
    full.push_str(&base);
    if !ext.is_empty() {
        full.push('.');
        full.push_str(&ext);
    }
    Some(full)
}

fn parse_name_component(bytes: &[u8]) -> Option<String> {
    let mut text = String::new();
    for byte in bytes {
        if *byte == b' ' || *byte == 0 {
            break;
        }
        if !(0x21..=0x7e).contains(byte) {
            return None;
        }
        text.push(*byte as char);
    }
    Some(text)
}

fn find_node<'a>(state: &'a FatState, path: &str) -> Option<&'a FatNode> {
    let normalized = normalize_fat_path(path)?;
    state.nodes.iter().find(|node| node.path == normalized)
}

fn render_directory_listing(nodes: &[FatNode], node: &FatNode) -> String {
    let mut text = String::new();
    for child_index in &node.children {
        let child = nodes.get(*child_index);
        if let Some(child) = child {
            let _ = writeln!(text, "{}", child.name);
        }
    }
    text
}

fn join_fat_path(parent: &str, name: &str) -> String {
    if parent == FAT_PATH_ROOT {
        let mut path = String::from(FAT_PATH_ROOT);
        path.push('/');
        path.push_str(name);
        return path;
    }

    let mut path = String::from(parent);
    path.push('/');
    path.push_str(name);
    path
}

fn normalize_fat_path(path: &str) -> Option<String> {
    if path == FAT_PATH_ROOT || path == "/fat/" {
        return Some(String::from(FAT_PATH_ROOT));
    }
    if !path.starts_with("/fat/") {
        return None;
    }

    let trimmed = path.trim_end_matches('/');
    let suffix = trimmed.strip_prefix("/fat/")?;
    if suffix.is_empty() {
        return Some(String::from(FAT_PATH_ROOT));
    }

    let mut normalized = String::from(FAT_PATH_ROOT);
    for component in suffix.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return None;
        }
        normalized.push('/');
        normalized.push_str(component);
    }
    Some(normalized)
}

fn fat_path_depth(path: &str) -> usize {
    let Some(relative) = path.strip_prefix("/fat") else {
        return 0;
    };
    relative
        .trim_matches('/')
        .split('/')
        .filter(|component| !component.is_empty())
        .count()
}

fn read_u16_le(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}

fn read_u32_le(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}
