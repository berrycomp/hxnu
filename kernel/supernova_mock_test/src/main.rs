use core::sync::atomic::{AtomicU32, Ordering};
use core::hint::spin_loop;
use core::ptr::{read_volatile, write_volatile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeBackend {
    Cuda, Vulkan, Metal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RTCoreCommand {
    RayTrace, MatrixMultiply,
}

#[derive(Debug, Clone)]
pub struct Hex2Payload { pub data: [u8; 32], pub size: usize }

#[derive(Debug, Clone)]
pub struct Hex4Payload { pub data: [u8; 64], pub size: usize }

#[derive(Debug, Clone)]
pub enum SxrcPayload {
    Hex2(Hex2Payload),
    Hex4(Hex4Payload),
}

#[derive(Debug, Clone)]
pub struct MartixPayload {
    pub backend: ComputeBackend,
    pub command: RTCoreCommand,
    pub payload: SxrcPayload,
}

#[repr(C)]
pub struct SupernovaHardware {
    pub doorbell: u32,
    pub completion: u32,
    pub ack: u32,
    pub _reserved: u32,
    pub ring_buffer: [u8; 4096],
}

static mut MOCK_HW: SupernovaHardware = SupernovaHardware {
    doorbell: 0,
    completion: 0,
    ack: 0,
    _reserved: 0,
    ring_buffer: [0; 4096],
};

pub struct SupernovaDriver {
    pub is_initialized: bool,
    pub seq_tracker: AtomicU32,
    mmio: *mut SupernovaHardware,
}

// We implement Send/Sync manually here just to allow the test to compile and prove the race condition.
// In the actual kernel, omitting this makes it impossible to share unless wrapped in static mut or unsafe blocks.
unsafe impl Send for SupernovaDriver {}
unsafe impl Sync for SupernovaDriver {}

impl SupernovaDriver {
    pub fn new() -> Self {
        Self {
            is_initialized: false,
            seq_tracker: AtomicU32::new(0),
            mmio: unsafe { &raw mut MOCK_HW as *mut _ },
        }
    }
    pub fn init(&mut self) { self.is_initialized = true; }
    pub fn ring_doorbell(&self, task_id: u32) {
        unsafe { write_volatile(&mut (*self.mmio).doorbell, task_id); }
    }
    pub fn wait_for_gsp(&self, expected_task_id: u32) {
        unsafe {
            while read_volatile(&(*self.mmio).completion) != expected_task_id {
                break; // avoid infinite loop in test
            }
        }
    }
    pub fn ack_task(&self, task_id: u32) {
        unsafe { write_volatile(&mut (*self.mmio).ack, task_id); }
    }
    fn write_to_ring_buffer(&self, offset: usize, data: &[u8]) {
        unsafe {
            let buffer_ptr = (*self.mmio).ring_buffer.as_mut_ptr();
            for (i, &byte) in data.iter().enumerate() {
                write_volatile(buffer_ptr.add(offset + i), byte);
            }
        }
    }
    pub fn route_sxrc(&mut self, payload: SxrcPayload) {
        let (size, data) = match &payload {
            SxrcPayload::Hex2(p) => (p.size, &p.data[..]),
            SxrcPayload::Hex4(p) => (p.size, &p.data[..]),
        };
        self.write_to_ring_buffer(0, &data[..size]);
        let route_id = size as u32;
        self.ring_doorbell(route_id);
    }
    pub fn route_martix(&self, payload: MartixPayload) {
        let (size, data) = match &payload.payload {
            SxrcPayload::Hex2(p) => (p.size, &p.data[..]),
            SxrcPayload::Hex4(p) => (p.size, &p.data[..]),
        };
        let backend_id = match payload.backend {
            ComputeBackend::Cuda => 1,
            ComputeBackend::Vulkan => 2,
            ComputeBackend::Metal => 3,
        };
        let command_id = match payload.command {
            RTCoreCommand::RayTrace => 0,
            RTCoreCommand::MatrixMultiply => 1,
        };
        self.write_to_ring_buffer(0, &[backend_id as u8, command_id as u8]);
        self.write_to_ring_buffer(2, &data[..size]);
        let task_id = (backend_id << 16) | (size as u32 & 0xFFFF);
        self.ring_doorbell(task_id);
    }
    pub fn synchronize(&self) {
        let sync_task_id = self.seq_tracker.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        self.ring_doorbell(sync_task_id);
        self.wait_for_gsp(sync_task_id);
        self.ack_task(sync_task_id);
    }
}

fn main() {
    let mut driver = SupernovaDriver::new();
    driver.init();

    println!("--- TEST 1: Memory Out of Bounds Crash ---");
    let payload = MartixPayload {
        backend: ComputeBackend::Cuda,
        command: RTCoreCommand::RayTrace,
        payload: SxrcPayload::Hex4(Hex4Payload {
            data: [0; 64],
            size: 100, // intentional out of bounds to trigger panic
        }),
    };
    
    let res = std::panic::catch_unwind(|| {
        driver.route_martix(payload);
    });
    if res.is_err() {
        println!("[SUCCESS] Panic caught on size > data.len() (100 > 64)!");
    } else {
        println!("[FAILED] No panic occurred.");
    }

    println!("\n--- TEST 2: Alignment Issue ---");
    // route_martix writes the payload to offset 2. This breaks 4-byte hardware alignment.
    let payload2 = MartixPayload {
        backend: ComputeBackend::Vulkan,
        command: RTCoreCommand::MatrixMultiply,
        payload: SxrcPayload::Hex2(Hex2Payload {
            data: [0x55; 32],
            size: 32,
        }),
    };
    driver.route_martix(payload2);
    unsafe {
        let ring_addr = core::ptr::addr_of!(MOCK_HW.ring_buffer) as usize;
        let payload_start_addr = ring_addr + 2;
        println!("Ring buffer base address: 0x{:X}", ring_addr);
        println!("Payload start address (offset 2): 0x{:X}", payload_start_addr);
        if payload_start_addr % 4 != 0 {
            println!("[CRITICAL] Payload is unaligned! Will crash GSP on 32-bit reads.");
        }
    }

    println!("\n--- TEST 3: Concurrent Data Race (MMIO doorbell & ring buffer) ---");
    use std::sync::Arc;
    let driver_arc = Arc::new(SupernovaDriver::new());
    
    let d1 = driver_arc.clone();
    let t1 = std::thread::spawn(move || {
        let p = MartixPayload {
            backend: ComputeBackend::Cuda,
            command: RTCoreCommand::RayTrace,
            payload: SxrcPayload::Hex2(Hex2Payload { data: [0xAA; 32], size: 32 }),
        };
        for _ in 0..1000 {
            d1.route_martix(p.clone());
        }
    });
    
    let d2 = driver_arc.clone();
    let t2 = std::thread::spawn(move || {
        let p = MartixPayload {
            backend: ComputeBackend::Vulkan,
            command: RTCoreCommand::MatrixMultiply,
            payload: SxrcPayload::Hex2(Hex2Payload { data: [0xBB; 32], size: 32 }),
        };
        for _ in 0..1000 {
            d2.route_martix(p.clone());
        }
    });
    
    t1.join().unwrap();
    t2.join().unwrap();

    unsafe {
        let d = MOCK_HW.doorbell;
        println!("[WARNING] Hardware Doorbell value at end: {}", d);
        println!("Concurrent writes to ring_buffer and doorbell without locks result in torn reads/writes by the GSP hardware.");
    }
}
