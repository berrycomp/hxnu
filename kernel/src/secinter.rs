//! Security Interceptor (SECinter) Subsystem
//!
//! This module computes an Entropy-Inclusive Syscall ID based on the payload of each
//! syscall to deterministically trace and log operations based on their data distribution
//! characteristics (Shannon Entropy) rather than random number generation.
//!
//! The 64-bit ID consists of:
//! - Hardware routing state / Thread ID (16 bits)
//! - Scaled Entropy Score (16 bits)
//! - FNV-1a Hash of the payload (32 bits)

use crate::sched;

/// Precomputed lookup table for `c * log2(c) * 1024` where `c` is the count of occurrences.
/// To avoid floating point operations (`no_std`), we scale the result.
/// `log2(c)` for c=0 is considered 0 because lim_{c->0} c * log2(c) = 0.
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

/// Computes a 32-bit FNV-1a hash of the given data.
fn fnv1a_32(data: &[u8]) -> u32 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Computes the scaled Shannon entropy of the given data.
/// Returns a 16-bit scaled entropy score.
fn compute_entropy(data: &[u8]) -> u16 {
    let mut counts = [0u8; 256];
    for &byte in data {
        counts[byte as usize] += 1;
    }
    
    let total_len = data.len();
    let mut sum_c_log_c = 0u32;
    for &count in &counts {
        if count > 0 {
            // max count is `data.len()`, which should be <= 64 based on the LUT size.
            let idx = core::cmp::min(count as usize, 64);
            sum_c_log_c += C_LOG_C_LUT[idx];
        }
    }
    
    let total_len_idx = core::cmp::min(total_len, 64);
    let total_log_total = C_LOG_C_LUT[total_len_idx];
    
    // Calculate scaled entropy: H = (total_log_total - sum_c_log_c) / total_len
    if total_len > 0 {
        let h_scaled = (total_log_total.saturating_sub(sum_c_log_c)) / (total_len as u32);
        h_scaled as u16
    } else {
        0
    }
}

/// Generates a deterministically calculated 64-bit Entropy-Inclusive Syscall ID.
/// Traces and logs the generated ID.
pub fn generate_syscall_entropy_id(number: u64, args: &[u64; 6]) -> u64 {
    let mut payload = [0u8; 56];
    payload[0..8].copy_from_slice(&number.to_ne_bytes());
    for i in 0..6 {
        let start = 8 + i * 8;
        payload[start..start+8].copy_from_slice(&args[i].to_ne_bytes());
    }
    
    let entropy_score = compute_entropy(&payload);
    let hash = fnv1a_32(&payload);
    
    // Fetch thread ID using fast-path getter and truncate to fit the 16-bit hardware routing state budget
    let tid = sched::current_thread_id();
    let thread_id = (tid & 0xFFFF) as u16;

    // 64-bit ID: Thread ID (16) | Entropy Score (16) | Hash (32)
    ((thread_id as u64) << 48) | ((entropy_score as u64) << 32) | (hash as u64)
}
