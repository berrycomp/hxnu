// TCOL / HPL (HXNU Public License)
// This file is strictly governed by the HXNU Public License (HPL).
const C_LOG_C_LUT: [u32; 65] = [
    0, 0, 2048, 4869, 8192, 11888, 15882, 20123, 24576, 29214, 34017, 38967, 44052,
    49260, 54582, 60010, 65536, 71155, 76860, 82648, 88513, 94452, 100462, 106539,
    112680, 118883, 125145, 131463, 137836, 144263, 150740, 157266, 163840, 170460,
    177125, 183834, 190584, 197376, 204207, 211078, 217986, 224931, 231913, 238929,
    245980, 253065, 260182, 267331, 274512, 281724, 288965, 296237, 303537, 310866,
    318222, 325606, 333017, 340454, 347917, 355406, 362919, 370458, 378020, 385606,
    393216
];

const FNV_PRIME: u32 = 16777619;
const FNV_OFFSET_BASIS: u32 = 2166136261;

fn fnv1a_32(data: &[u8]) -> u32 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn compute_entropy(data: &[u8]) -> u16 {
    let mut counts = [0u8; 256];
    for &byte in data {
        counts[byte as usize] += 1;
    }
    
    let total_len = data.len();
    let mut sum_c_log_c = 0u32;
    for &count in &counts {
        if count > 0 {
            let idx = core::cmp::min(count as usize, 64);
            sum_c_log_c += C_LOG_C_LUT[idx];
        }
    }
    
    let total_len_idx = core::cmp::min(total_len, 64);
    let total_log_total = C_LOG_C_LUT[total_len_idx];
    
    if total_len > 0 {
        let h_scaled = (total_log_total.saturating_sub(sum_c_log_c)) / (total_len as u32);
        h_scaled as u16
    } else {
        0
    }
}

fn generate_syscall_entropy_id(number: u64, args: &[u64; 6]) -> u64 {
    let mut payload = [0u8; 56];
    payload[0..8].copy_from_slice(&number.to_ne_bytes());
    for i in 0..6 {
        let start = 8 + i * 8;
        payload[start..start+8].copy_from_slice(&args[i].to_ne_bytes());
    }
    
    let entropy_score = compute_entropy(&payload);
    let hash = fnv1a_32(&payload);
    
    let thread_id = 42u16; // Dummy
    
    ((thread_id as u64) << 48) | ((entropy_score as u64) << 32) | (hash as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_zero() {
        let args = [0u64; 6];
        let id = generate_syscall_entropy_id(0, &args);
        let entropy = (id >> 32) & 0xFFFF;
        assert_eq!(entropy, 0, "Entropy of all zeros should be 0");
    }

    #[test]
    fn test_entropy_ones() {
        let args = [u64::MAX; 6];
        let id = generate_syscall_entropy_id(u64::MAX, &args);
        let entropy = (id >> 32) & 0xFFFF;
        assert_eq!(entropy, 0, "Entropy of all ones should be 0");
    }
    
    #[test]
    fn test_entropy_half_split() {
        let mut args = [0u64; 6];
        // 56 bytes total. 28 of 0s, 28 of 1s
        // number = 8 bytes. args = 48 bytes.
        // number = all 0s.
        // args[0..2] = all 0s. 16 bytes.
        // args[2..3] = 4 bytes of 0, 4 bytes of 1.
        // Wait, easier to test compute_entropy directly.
        let mut data = [0u8; 56];
        for i in 0..28 { data[i] = 0; }
        for i in 28..56 { data[i] = 255; }
        
        let entropy = compute_entropy(&data);
        assert_eq!(entropy, 1024, "Entropy of 50-50 split should be exactly 1024 (1 bit)");
    }
    
    #[test]
    fn test_fnv1a_empty() {
        assert_eq!(fnv1a_32(&[]), FNV_OFFSET_BASIS);
    }
}

fn main() {}
