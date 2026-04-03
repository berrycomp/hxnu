#![allow(dead_code)]

use core::cell::UnsafeCell;

const MAX_ACCEL_DRIVERS: usize = 4;
const SPE_STUB_QUEUE_DEPTH: usize = 32;

#[derive(Copy, Clone)]
pub enum AccelKind {
    Spe,
}

impl AccelKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Spe => "spe",
        }
    }
}

#[derive(Copy, Clone)]
pub struct AccelCaps {
    pub mailbox: bool,
    pub dma: bool,
    pub queue_depth: u16,
}

impl AccelCaps {
    const fn empty() -> Self {
        Self {
            mailbox: false,
            dma: false,
            queue_depth: 0,
        }
    }
}

#[derive(Copy, Clone)]
pub struct AccelJobEnvelope {
    pub job_id: u64,
    pub code_address: u64,
    pub code_length: u32,
    pub data_address: u64,
    pub data_length: u32,
    pub flags: u32,
}

#[derive(Copy, Clone)]
pub struct AccelCompletion {
    pub job_id: u64,
    pub status: u32,
}

#[derive(Copy, Clone, Debug)]
pub enum AccelSubmitError {
    NotInitialized,
    InvalidJob,
    QueueFull,
}

impl AccelSubmitError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotInitialized => "accelerator subsystem is not initialized",
            Self::InvalidJob => "job envelope is invalid",
            Self::QueueFull => "accelerator queue is full",
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum AccelInitError {
    AlreadyInitialized,
}

impl AccelInitError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "accelerator subsystem is already initialized",
        }
    }
}

#[derive(Copy, Clone)]
pub struct AccelDriverOps {
    pub driver_name: &'static str,
    pub kind: AccelKind,
    pub probe: fn() -> bool,
    pub caps: fn() -> AccelCaps,
    pub submit: fn(AccelJobEnvelope) -> Result<(), AccelSubmitError>,
    pub poll_complete: fn() -> Option<AccelCompletion>,
    pub cancel: fn(job_id: u64) -> bool,
}

#[derive(Copy, Clone)]
struct DriverSlot {
    present: bool,
    ops: AccelDriverOps,
}

impl DriverSlot {
    const fn empty() -> Self {
        Self {
            present: false,
            ops: AccelDriverOps {
                driver_name: "",
                kind: AccelKind::Spe,
                probe: spe_stub_probe,
                caps: spe_stub_caps,
                submit: spe_stub_submit,
                poll_complete: spe_stub_poll_complete,
                cancel: spe_stub_cancel,
            },
        }
    }
}

#[derive(Copy, Clone)]
pub struct AccelSummary {
    pub initialized: bool,
    pub driver_count: usize,
    pub pending_jobs: usize,
    pub submitted_jobs: u64,
    pub completed_jobs: u64,
    pub canceled_jobs: u64,
}

impl AccelSummary {
    const fn empty() -> Self {
        Self {
            initialized: false,
            driver_count: 0,
            pending_jobs: 0,
            submitted_jobs: 0,
            completed_jobs: 0,
            canceled_jobs: 0,
        }
    }
}

struct SpeStubState {
    queue: [Option<AccelJobEnvelope>; SPE_STUB_QUEUE_DEPTH],
    pending: usize,
    submitted: u64,
    completed: u64,
    canceled: u64,
}

impl SpeStubState {
    const fn new() -> Self {
        Self {
            queue: [None; SPE_STUB_QUEUE_DEPTH],
            pending: 0,
            submitted: 0,
            completed: 0,
            canceled: 0,
        }
    }

    fn submit(&mut self, job: AccelJobEnvelope) -> Result<(), AccelSubmitError> {
        if job.job_id == 0 {
            return Err(AccelSubmitError::InvalidJob);
        }
        if self.pending >= SPE_STUB_QUEUE_DEPTH {
            return Err(AccelSubmitError::QueueFull);
        }

        for slot in self.queue.iter_mut() {
            if slot.is_none() {
                *slot = Some(job);
                self.pending += 1;
                self.submitted = self.submitted.saturating_add(1);
                return Ok(());
            }
        }

        Err(AccelSubmitError::QueueFull)
    }

    fn poll_complete(&mut self) -> Option<AccelCompletion> {
        for slot in self.queue.iter_mut() {
            if let Some(job) = slot.take() {
                self.pending = self.pending.saturating_sub(1);
                self.completed = self.completed.saturating_add(1);
                return Some(AccelCompletion {
                    job_id: job.job_id,
                    status: 0,
                });
            }
        }
        None
    }

    fn cancel(&mut self, job_id: u64) -> bool {
        for slot in self.queue.iter_mut() {
            if slot.as_ref().is_some_and(|job| job.job_id == job_id) {
                *slot = None;
                self.pending = self.pending.saturating_sub(1);
                self.canceled = self.canceled.saturating_add(1);
                return true;
            }
        }
        false
    }
}

struct AccelState {
    initialized: bool,
    driver_count: usize,
    drivers: [DriverSlot; MAX_ACCEL_DRIVERS],
    spe_stub: SpeStubState,
}

impl AccelState {
    const fn new() -> Self {
        Self {
            initialized: false,
            driver_count: 0,
            drivers: [DriverSlot::empty(); MAX_ACCEL_DRIVERS],
            spe_stub: SpeStubState::new(),
        }
    }

    fn initialize(&mut self) -> Result<AccelSummary, AccelInitError> {
        if self.initialized {
            return Err(AccelInitError::AlreadyInitialized);
        }

        self.initialized = true;
        self.register_driver(AccelDriverOps {
            driver_name: "spe-stub",
            kind: AccelKind::Spe,
            probe: spe_stub_probe,
            caps: spe_stub_caps,
            submit: spe_stub_submit,
            poll_complete: spe_stub_poll_complete,
            cancel: spe_stub_cancel,
        });
        Ok(self.summary())
    }

    fn register_driver(&mut self, ops: AccelDriverOps) {
        for slot in self.drivers.iter_mut() {
            if slot.present {
                continue;
            }
            slot.present = true;
            slot.ops = ops;
            self.driver_count += 1;
            break;
        }
    }

    fn summary(&self) -> AccelSummary {
        AccelSummary {
            initialized: self.initialized,
            driver_count: self.driver_count,
            pending_jobs: self.spe_stub.pending,
            submitted_jobs: self.spe_stub.submitted,
            completed_jobs: self.spe_stub.completed,
            canceled_jobs: self.spe_stub.canceled,
        }
    }
}

struct GlobalAccel(UnsafeCell<AccelState>);

unsafe impl Sync for GlobalAccel {}

impl GlobalAccel {
    const fn new() -> Self {
        Self(UnsafeCell::new(AccelState::new()))
    }

    fn get(&self) -> *mut AccelState {
        self.0.get()
    }
}

static ACCEL: GlobalAccel = GlobalAccel::new();

pub fn initialize() -> Result<AccelSummary, AccelInitError> {
    unsafe { (&mut *ACCEL.get()).initialize() }
}

pub fn is_initialized() -> bool {
    unsafe { (&*ACCEL.get()).initialized }
}

pub fn summary() -> AccelSummary {
    unsafe { (&*ACCEL.get()).summary() }
}

pub fn driver(index: usize) -> Option<AccelDriverOps> {
    let state = unsafe { &*ACCEL.get() };
    if index >= MAX_ACCEL_DRIVERS {
        return None;
    }
    let slot = state.drivers[index];
    if slot.present { Some(slot.ops) } else { None }
}

pub fn driver_count() -> usize {
    unsafe { (&*ACCEL.get()).driver_count }
}

pub fn submit(job: AccelJobEnvelope) -> Result<(), AccelSubmitError> {
    let state = unsafe { &mut *ACCEL.get() };
    if !state.initialized {
        return Err(AccelSubmitError::NotInitialized);
    }
    spe_stub_submit(job)
}

pub fn poll_complete() -> Option<AccelCompletion> {
    if !is_initialized() {
        return None;
    }
    spe_stub_poll_complete()
}

pub fn cancel(job_id: u64) -> bool {
    if !is_initialized() {
        return false;
    }
    spe_stub_cancel(job_id)
}

fn spe_stub_probe() -> bool {
    true
}

fn spe_stub_caps() -> AccelCaps {
    AccelCaps {
        mailbox: true,
        dma: true,
        queue_depth: SPE_STUB_QUEUE_DEPTH as u16,
    }
}

fn spe_stub_submit(job: AccelJobEnvelope) -> Result<(), AccelSubmitError> {
    unsafe { (&mut *ACCEL.get()).spe_stub.submit(job) }
}

fn spe_stub_poll_complete() -> Option<AccelCompletion> {
    unsafe { (&mut *ACCEL.get()).spe_stub.poll_complete() }
}

fn spe_stub_cancel(job_id: u64) -> bool {
    unsafe { (&mut *ACCEL.get()).spe_stub.cancel(job_id) }
}
