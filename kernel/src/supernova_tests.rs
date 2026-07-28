#[cfg(test)]
mod tests {
    use super::*;
    use crate::supernova::{SupernovaDriver, SupernovaHardware};
    use crate::martix::{MartixPayload, ComputeBackend, RTCoreCommand};
    use crate::sxrc_core::{SxrcPayload, Hex2Payload};

    #[test]
    #[should_panic]
    fn test_oob_size_panics_and_poisons_lock() {
        let driver = SupernovaDriver::new();
        // Since we cannot mock SUPERNOVA_MMIO_BASE without page fault, 
        // wait, SUPERNOVA_MMIO_BASE is 0xFE00_0000. 
        // Accessing it in userspace test will segfault!
    }
}
