// THOL (Turkish Hybrid Open License)
// This file is strictly governed by the Turkish Hybrid Open License (THOL).
#[cfg(test)]
mod tests {
    use crate::supernova::{SupernovaDriver, SupernovaHardware};
    use crate::martix::{MartixPayload, ComputeBackend, RTCoreCommand};
    use crate::sxrc_core::{SxrcPayload, Hex2Payload};

    extern "C" {
        fn mmap(addr: *mut core::ffi::c_void, length: usize, prot: i32, flags: i32, fd: i32, offset: isize) -> *mut core::ffi::c_void;
    }

    fn setup_mmio() {
        unsafe {
            let addr = 0xFE00_0000 as *mut core::ffi::c_void;
            let res = mmap(addr, 4096, 3, 0x22, -1, 0); // PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED
            assert_eq!(res, addr, "Failed to mmap MMIO base");
        }
    }

    #[test]
    fn test_route_sxrc_oob_size_no_panic() {
        setup_mmio();
        let driver = SupernovaDriver::new();
        let payload = SxrcPayload::Hex2(Hex2Payload {
            data: [0; 32],
            size: 100, // Invalid size
        });
        driver.route_sxrc(payload);
    }
}
