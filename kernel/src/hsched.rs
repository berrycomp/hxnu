///! Heterogeneous Scheduler (HXNU)
///! Capable of managing asymmetric workloads across CPU (AVX-512) and GPU (RT/Tensor Cores).
///! Relies on the bare-metal HPS for zero-latency dispatching to prevent driver timeouts.

use core::sync::atomic::{AtomicUsize, AtomicU32, AtomicU64, Ordering};
use crate::supernova::SupernovaDriver;

/// Represents an abstract asymmetric task in the Neonix Operating System context.
pub struct AsymmetricTask {
    id: u32,
    workload_type: WorkloadType,
}

/// Represents a hardware task or command that will be scheduled.
pub struct HardwareTask {
    /// The ID of the task.
    pub id: u32,
    /// Indicates whether the task is targeted for GPU or CPU execution.
    pub is_gpu_task: bool,
}

impl HardwareTask {
    pub fn encode(&self) -> u64 {
        let valid = 1u64 << 63;
        let gpu = if self.is_gpu_task { 1u64 << 32 } else { 0 };
        valid | gpu | (self.id as u64)
    }

    pub fn decode(val: u64) -> Option<Self> {
        if val & (1u64 << 63) == 0 {
            None
        } else {
            Some(Self {
                id: (val & 0xFFFFFFFF) as u32,
                is_gpu_task: (val & (1u64 << 32)) != 0,
            })
        }
    }
}

/// The type of the workload, determining where it gets dispatched.
pub enum WorkloadType {
    /// CPU intensive workload, preferring AVX-512 capabilities.
    CpuAvx512,
    /// GPU intensive workload, targeting RT/Tensor Cores.
    GpuCompute,
    /// Accelerated SXRC payload offloaded to the GPU/Supernova driver.
    Sxrc(crate::sxrc_core::SxrcPayload),
}

#[repr(C)]
pub struct Node {
    pub sequence: AtomicUsize,
    pub data: AtomicU64,
}

#[repr(C)]
pub struct SharedRingBuffer {
    pub buffer: [Node; 16],
    pub head: AtomicUsize,
    pub tail: AtomicUsize,
}

/// The Heterogeneous Scheduler for managing asymmetric workloads.
pub struct HeterogeneousScheduler {
    /// The integrated Supernova driver for GPU and accelerator tasks.
    supernova: SupernovaDriver,
    next_id: AtomicU32,
    /// Shared Queue for pending hardware tasks to be polled by the .hxext layer
    pub shared_buffer: SharedRingBuffer,
}

impl HeterogeneousScheduler {
    /// Initializes a new HeterogeneousScheduler.
    pub const fn new() -> Self {
        const INIT_NODE: Node = Node { sequence: AtomicUsize::new(0), data: AtomicU64::new(0) };
        HeterogeneousScheduler {
            supernova: SupernovaDriver::new(),
            next_id: AtomicU32::new(1),
            shared_buffer: SharedRingBuffer {
                buffer: [
                    Node { sequence: AtomicUsize::new(0), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(1), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(2), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(3), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(4), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(5), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(6), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(7), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(8), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(9), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(10), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(11), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(12), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(13), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(14), data: AtomicU64::new(0) },
                    Node { sequence: AtomicUsize::new(15), data: AtomicU64::new(0) },
                ],
                head: AtomicUsize::new(0),
                tail: AtomicUsize::new(0),
            },
        }
    }

    /// Initializes the underlying hardware drivers.
    pub fn init(&self) {
        self.supernova.init();
    }

    /// Submits a workload to the heterogeneous scheduler.
    /// It translates the abstract task to a direct hardware task and
    /// bypasses Neonix OS blocking to prevent driver timeouts.
    pub fn submit_workload(&self, workload: WorkloadType) -> Result<u32, &'static str> {
        let task_id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let is_gpu = match workload {
            WorkloadType::CpuAvx512 => false,
            WorkloadType::GpuCompute => true,
            WorkloadType::Sxrc(_) => true,
        };

        let hw_task = HardwareTask {
            id: task_id,
            is_gpu_task: is_gpu,
        };

        let mut pos = self.shared_buffer.tail.load(Ordering::Relaxed);
        loop {
            let index = pos % 16;
            let seq = self.shared_buffer.buffer[index].sequence.load(Ordering::Acquire);
            
            let dif = seq.wrapping_sub(pos) as isize;
            
            if dif == 0 {
                match self.shared_buffer.tail.compare_exchange_weak(
                    pos, pos.wrapping_add(1),
                    Ordering::SeqCst, Ordering::Relaxed
                ) {
                    Ok(_) => {
                        let payload = hw_task.encode();
                        self.shared_buffer.buffer[index].data.store(payload, Ordering::Relaxed);
                        self.shared_buffer.buffer[index].sequence.store(pos.wrapping_add(1), Ordering::Release);
                        break;
                    }
                    Err(actual) => {
                        pos = actual;
                    }
                }
            } else if dif < 0 {
                return Err("Task queue is full");
            } else {
                pos = self.shared_buffer.tail.load(Ordering::Relaxed);
            }
        }
        Ok(task_id)
    }

    /// Tries to dispatch pending workloads, exposing them to HPS or direct drivers.
    pub fn dispatch_pending(&self) {
        let mut pos = self.shared_buffer.head.load(Ordering::Relaxed);
        loop {
            let index = pos % 16;
            let seq = self.shared_buffer.buffer[index].sequence.load(Ordering::Acquire);
            
            let dif = seq.wrapping_sub(pos.wrapping_add(1)) as isize;
            
            if dif == 0 {
                match self.shared_buffer.head.compare_exchange_weak(
                    pos, pos.wrapping_add(1),
                    Ordering::SeqCst, Ordering::Relaxed
                ) {
                    Ok(_) => {
                        let val = self.shared_buffer.buffer[index].data.load(Ordering::Relaxed);
                        self.shared_buffer.buffer[index].sequence.store(pos.wrapping_add(16), Ordering::Release);
                        
                        if let Some(task) = HardwareTask::decode(val) {
                            if task.is_gpu_task {
                                self.supernova.ring_doorbell(task.id);
                            }
                        }
                        pos = pos.wrapping_add(1);
                    }
                    Err(actual) => {
                        pos = actual;
                    }
                }
            } else if dif < 0 {
                break;
            } else {
                pos = self.shared_buffer.head.load(Ordering::Relaxed);
            }
        }
    }
}

/// Global instance of the heterogeneous scheduler (for stub purposes).
pub static HSCHED: HeterogeneousScheduler = HeterogeneousScheduler::new();
