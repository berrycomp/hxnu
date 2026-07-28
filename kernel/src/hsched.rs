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
pub struct SharedRingBuffer {
    pub pending_tasks: [AtomicU64; 16],
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
        const INIT_TASK: AtomicU64 = AtomicU64::new(0);
        HeterogeneousScheduler {
            supernova: SupernovaDriver::new(),
            next_id: AtomicU32::new(1),
            shared_buffer: SharedRingBuffer {
                pending_tasks: [INIT_TASK; 16],
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

        let mut current_tail = self.shared_buffer.tail.load(Ordering::Acquire);
        loop {
            let current_head = self.shared_buffer.head.load(Ordering::Acquire);
            let next_tail = (current_tail + 1) % self.shared_buffer.pending_tasks.len();
            if next_tail == current_head {
                return Err("Task queue is full");
            }
            match self.shared_buffer.pending_tasks[current_tail].compare_exchange_weak(
                0,
                hw_task.encode(),
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let _ = self.shared_buffer.tail.compare_exchange_weak(
                        current_tail,
                        next_tail,
                        Ordering::SeqCst,
                        Ordering::Relaxed,
                    );
                    break;
                }
                Err(_) => {
                    let global_tail = self.shared_buffer.tail.load(Ordering::Acquire);
                    current_tail = if global_tail != current_tail {
                        global_tail
                    } else {
                        next_tail
                    };
                }
            }
        }
        Ok(task_id)
    }

    /// Tries to dispatch pending workloads, exposing them to HPS or direct drivers.
    pub fn dispatch_pending(&self) {
        let mut current_head = self.shared_buffer.head.load(Ordering::Acquire);
        loop {
            let current_tail = self.shared_buffer.tail.load(Ordering::Acquire);
            if current_head == current_tail {
                break;
            }
            let val = self.shared_buffer.pending_tasks[current_head].load(Ordering::Acquire);
            if val == 0 {
                let global_head = self.shared_buffer.head.load(Ordering::Acquire);
                current_head = if global_head != current_head {
                    global_head
                } else {
                    (current_head + 1) % self.shared_buffer.pending_tasks.len()
                };
                continue;
            }
            let next_head = (current_head + 1) % self.shared_buffer.pending_tasks.len();
            match self.shared_buffer.pending_tasks[current_head].compare_exchange_weak(
                val,
                0,
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let _ = self.shared_buffer.head.compare_exchange_weak(
                        current_head,
                        next_head,
                        Ordering::SeqCst,
                        Ordering::Relaxed,
                    );
                    if let Some(task) = HardwareTask::decode(val) {
                        if task.is_gpu_task {
                            self.supernova.ring_doorbell(task.id);
                        }
                    }
                    current_head = next_head;
                }
                Err(_) => {
                    let global_head = self.shared_buffer.head.load(Ordering::Acquire);
                    current_head = if global_head != current_head {
                        global_head
                    } else {
                        next_head
                    };
                }
            }
        }
    }
}

/// Global instance of the heterogeneous scheduler (for stub purposes).
pub static HSCHED: HeterogeneousScheduler = HeterogeneousScheduler::new();
