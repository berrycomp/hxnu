use core::cell::UnsafeCell;

use super::{CompressionClass, CompressionError, EncodedPage, MAX_ENCODED_PAGE_BYTES, PAGE_BYTES};

pub const STORE_CAPACITY_PAGES: usize = 64;
pub const STORE_CAPACITY_BYTES: usize = STORE_CAPACITY_PAGES * MAX_ENCODED_PAGE_BYTES;

#[derive(Copy, Clone)]
pub struct StoreSummary {
    pub capacity_pages: usize,
    pub capacity_bytes: usize,
    pub page_bytes: usize,
    pub max_encoded_page_bytes: usize,
}

#[derive(Copy, Clone, Debug)]
pub enum StoreInitError {
    AlreadyInitialized,
    CompressionRuntimeUnavailable,
}

impl StoreInitError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "compression store is already initialized",
            Self::CompressionRuntimeUnavailable => "compression runtime must be initialized before store",
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum StoreError {
    NotInitialized,
    NotFound,
    Encode(CompressionError),
    Decode(CompressionError),
}

impl StoreError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotInitialized => "compression store is not initialized",
            Self::NotFound => "page id is not present in compressed store",
            Self::Encode(error) => error.as_str(),
            Self::Decode(error) => error.as_str(),
        }
    }
}

#[derive(Copy, Clone)]
pub struct StoreEntry {
    pub page_id: u64,
    pub class: CompressionClass,
    pub encoded_bytes: usize,
}

#[derive(Copy, Clone)]
pub struct StoreStats {
    pub capacity_pages: usize,
    pub capacity_bytes: usize,
    pub stored_pages: u64,
    pub stored_zero_pages: u64,
    pub stored_same_pages: u64,
    pub stored_sxrc_pages: u64,
    pub stored_raw_pages: u64,
    pub current_encoded_bytes: u64,
    pub total_input_bytes: u64,
    pub total_encoded_bytes: u64,
    pub store_requests: u64,
    pub store_successes: u64,
    pub load_requests: u64,
    pub load_successes: u64,
    pub load_misses: u64,
    pub replacements: u64,
    pub evictions: u64,
    pub encode_failures: u64,
    pub decode_failures: u64,
}

impl StoreStats {
    const fn new() -> Self {
        Self {
            capacity_pages: STORE_CAPACITY_PAGES,
            capacity_bytes: STORE_CAPACITY_BYTES,
            stored_pages: 0,
            stored_zero_pages: 0,
            stored_same_pages: 0,
            stored_sxrc_pages: 0,
            stored_raw_pages: 0,
            current_encoded_bytes: 0,
            total_input_bytes: 0,
            total_encoded_bytes: 0,
            store_requests: 0,
            store_successes: 0,
            load_requests: 0,
            load_successes: 0,
            load_misses: 0,
            replacements: 0,
            evictions: 0,
            encode_failures: 0,
            decode_failures: 0,
        }
    }
}

#[derive(Copy, Clone)]
struct StoreSlot {
    occupied: bool,
    page_id: u64,
    class: CompressionClass,
    encoded_len: u16,
    encoded: [u8; MAX_ENCODED_PAGE_BYTES],
}

impl StoreSlot {
    const fn empty() -> Self {
        Self {
            occupied: false,
            page_id: 0,
            class: CompressionClass::Raw,
            encoded_len: 0,
            encoded: [0; MAX_ENCODED_PAGE_BYTES],
        }
    }
}

struct StoreState {
    initialized: bool,
    next_evict: usize,
    slots: [StoreSlot; STORE_CAPACITY_PAGES],
    stats: StoreStats,
}

impl StoreState {
    const fn new() -> Self {
        Self {
            initialized: false,
            next_evict: 0,
            slots: [StoreSlot::empty(); STORE_CAPACITY_PAGES],
            stats: StoreStats::new(),
        }
    }

    fn initialize(&mut self) -> Result<StoreSummary, StoreInitError> {
        if self.initialized {
            return Err(StoreInitError::AlreadyInitialized);
        }
        if !super::is_initialized() {
            return Err(StoreInitError::CompressionRuntimeUnavailable);
        }
        self.initialized = true;
        Ok(self.summary())
    }

    fn summary(&self) -> StoreSummary {
        StoreSummary {
            capacity_pages: STORE_CAPACITY_PAGES,
            capacity_bytes: STORE_CAPACITY_BYTES,
            page_bytes: PAGE_BYTES,
            max_encoded_page_bytes: MAX_ENCODED_PAGE_BYTES,
        }
    }

    fn store_page(&mut self, page_id: u64, page: &[u8; PAGE_BYTES]) -> Result<StoreEntry, StoreError> {
        if !self.initialized {
            return Err(StoreError::NotInitialized);
        }

        self.stats.store_requests = self.stats.store_requests.saturating_add(1);
        self.stats.total_input_bytes = self.stats.total_input_bytes.saturating_add(PAGE_BYTES as u64);

        let mut scratch = [0u8; MAX_ENCODED_PAGE_BYTES];
        let encoded = super::encode_page(page, &mut scratch).map_err(|error| {
            self.stats.encode_failures = self.stats.encode_failures.saturating_add(1);
            StoreError::Encode(error)
        })?;
        let encoded_class = encoded.class();
        let encoded_bytes = encoded.bytes();
        let encoded_len = encoded_bytes.len();

        let existing_idx = self.find_slot(page_id);
        let target_idx = if let Some(idx) = existing_idx {
            self.stats.replacements = self.stats.replacements.saturating_add(1);
            idx
        } else if let Some(idx) = self.find_empty_slot() {
            idx
        } else {
            let idx = self.next_evict;
            self.next_evict = (self.next_evict + 1) % STORE_CAPACITY_PAGES;
            self.stats.evictions = self.stats.evictions.saturating_add(1);
            idx
        };

        let (had_old, old_class, old_len) = {
            let slot = &self.slots[target_idx];
            (slot.occupied, slot.class, usize::from(slot.encoded_len))
        };
        if had_old {
            self.account_removed(old_class, old_len);
        } else {
            self.stats.stored_pages = self.stats.stored_pages.saturating_add(1);
        }

        {
            let slot = &mut self.slots[target_idx];
            slot.occupied = true;
            slot.page_id = page_id;
            slot.class = encoded_class;
            slot.encoded_len = encoded_len as u16;
            slot.encoded[..encoded_len].copy_from_slice(encoded_bytes);
        }

        self.account_added(encoded_class, encoded_len);
        self.stats.store_successes = self.stats.store_successes.saturating_add(1);
        self.stats.total_encoded_bytes = self.stats.total_encoded_bytes.saturating_add(encoded_len as u64);

        Ok(StoreEntry {
            page_id,
            class: encoded_class,
            encoded_bytes: encoded_len,
        })
    }

    fn load_page(&mut self, page_id: u64, out: &mut [u8; PAGE_BYTES]) -> Result<StoreEntry, StoreError> {
        if !self.initialized {
            return Err(StoreError::NotInitialized);
        }

        self.stats.load_requests = self.stats.load_requests.saturating_add(1);
        let Some(slot_idx) = self.find_slot(page_id) else {
            self.stats.load_misses = self.stats.load_misses.saturating_add(1);
            return Err(StoreError::NotFound);
        };

        let (class, encoded_len, decode_result) = {
            let slot = &self.slots[slot_idx];
            let encoded_len = usize::from(slot.encoded_len);
            let encoded = EncodedPage::new(slot.class, &slot.encoded[..encoded_len]);
            (slot.class, encoded_len, super::decode_page(encoded, out))
        };

        match decode_result {
            Ok(()) => {
                self.stats.load_successes = self.stats.load_successes.saturating_add(1);
                Ok(StoreEntry {
                    page_id,
                    class,
                    encoded_bytes: encoded_len,
                })
            }
            Err(error) => {
                self.stats.decode_failures = self.stats.decode_failures.saturating_add(1);
                Err(StoreError::Decode(error))
            }
        }
    }

    fn stats(&self) -> StoreStats {
        self.stats
    }

    fn account_added(&mut self, class: CompressionClass, encoded_len: usize) {
        self.stats.current_encoded_bytes = self.stats.current_encoded_bytes.saturating_add(encoded_len as u64);
        match class {
            CompressionClass::Zero => {
                self.stats.stored_zero_pages = self.stats.stored_zero_pages.saturating_add(1);
            }
            CompressionClass::Same => {
                self.stats.stored_same_pages = self.stats.stored_same_pages.saturating_add(1);
            }
            CompressionClass::Sxrc => {
                self.stats.stored_sxrc_pages = self.stats.stored_sxrc_pages.saturating_add(1);
            }
            CompressionClass::Raw => {
                self.stats.stored_raw_pages = self.stats.stored_raw_pages.saturating_add(1);
            }
        }
    }

    fn account_removed(&mut self, class: CompressionClass, encoded_len: usize) {
        self.stats.current_encoded_bytes = self.stats.current_encoded_bytes.saturating_sub(encoded_len as u64);
        match class {
            CompressionClass::Zero => {
                self.stats.stored_zero_pages = self.stats.stored_zero_pages.saturating_sub(1);
            }
            CompressionClass::Same => {
                self.stats.stored_same_pages = self.stats.stored_same_pages.saturating_sub(1);
            }
            CompressionClass::Sxrc => {
                self.stats.stored_sxrc_pages = self.stats.stored_sxrc_pages.saturating_sub(1);
            }
            CompressionClass::Raw => {
                self.stats.stored_raw_pages = self.stats.stored_raw_pages.saturating_sub(1);
            }
        }
    }

    fn find_slot(&self, page_id: u64) -> Option<usize> {
        let mut index = 0usize;
        while index < STORE_CAPACITY_PAGES {
            let slot = &self.slots[index];
            if slot.occupied && slot.page_id == page_id {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn find_empty_slot(&self) -> Option<usize> {
        let mut index = 0usize;
        while index < STORE_CAPACITY_PAGES {
            if !self.slots[index].occupied {
                return Some(index);
            }
            index += 1;
        }
        None
    }
}

struct GlobalStore(UnsafeCell<StoreState>);

unsafe impl Sync for GlobalStore {}

impl GlobalStore {
    const fn new() -> Self {
        Self(UnsafeCell::new(StoreState::new()))
    }

    fn get(&self) -> *mut StoreState {
        self.0.get()
    }
}

static STORE: GlobalStore = GlobalStore::new();

pub fn initialize() -> Result<StoreSummary, StoreInitError> {
    unsafe { (*STORE.get()).initialize() }
}

pub fn summary() -> StoreSummary {
    unsafe { (*STORE.get()).summary() }
}

pub fn is_initialized() -> bool {
    unsafe { (*STORE.get()).initialized }
}

pub fn store_page(page_id: u64, page: &[u8; PAGE_BYTES]) -> Result<StoreEntry, StoreError> {
    unsafe { (*STORE.get()).store_page(page_id, page) }
}

pub fn load_page(page_id: u64, out: &mut [u8; PAGE_BYTES]) -> Result<StoreEntry, StoreError> {
    unsafe { (*STORE.get()).load_page(page_id, out) }
}

pub fn stats() -> StoreStats {
    unsafe { (*STORE.get()).stats() }
}
