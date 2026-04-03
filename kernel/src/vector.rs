use alloc::string::String;
use core::cell::UnsafeCell;
use core::fmt::Write;

#[cfg(target_arch = "x86_64")]
use core::arch::asm;

#[cfg(target_arch = "x86_64")]
use crate::arch;

pub struct VectorFeatureBits;

impl VectorFeatureBits {
    pub const AVX: u64 = 1 << 0;
    pub const AVX2: u64 = 1 << 1;
    pub const AVX512F: u64 = 1 << 2;
    pub const NEON: u64 = 1 << 3;
    pub const ALTIVEC: u64 = 1 << 4;
    pub const VSX: u64 = 1 << 5;

    pub const AVX512BW: u64 = 1 << 0;
    pub const AVX512DQ: u64 = 1 << 1;
    pub const AVX512VL: u64 = 1 << 2;
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum VectorContextMode {
    ScalarOnly,
    Fxsave,
    Xsave,
    Aarch64FpSimd,
    Ppc64Vsx,
}

impl VectorContextMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScalarOnly => "scalar-only",
            Self::Fxsave => "fxsave",
            Self::Xsave => "xsave",
            Self::Aarch64FpSimd => "aarch64-fpsimd",
            Self::Ppc64Vsx => "ppc64-vsx",
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum VectorContextPolicy {
    Eager,
    ScalarFallback,
}

impl VectorContextPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Eager => "eager",
            Self::ScalarFallback => "scalar-fallback",
        }
    }
}

#[derive(Copy, Clone)]
pub struct VectorCaps {
    pub initialized: bool,
    pub base_bits: u64,
    pub ext_bits: u64,
    pub xsave_mask: u64,
    pub context_mode: VectorContextMode,
    pub policy: VectorContextPolicy,
}

impl VectorCaps {
    pub const fn empty() -> Self {
        Self {
            initialized: false,
            base_bits: 0,
            ext_bits: 0,
            xsave_mask: 0,
            context_mode: VectorContextMode::ScalarOnly,
            policy: VectorContextPolicy::ScalarFallback,
        }
    }

    pub const fn has_base(self, bit: u64) -> bool {
        self.base_bits & bit != 0
    }

    pub const fn has_ext(self, bit: u64) -> bool {
        self.ext_bits & bit != 0
    }
}

struct GlobalVector(UnsafeCell<VectorCaps>);

unsafe impl Sync for GlobalVector {}

impl GlobalVector {
    const fn new() -> Self {
        Self(UnsafeCell::new(VectorCaps::empty()))
    }

    fn get(&self) -> *mut VectorCaps {
        self.0.get()
    }
}

static VECTOR_CAPS: GlobalVector = GlobalVector::new();

pub fn initialize() -> VectorCaps {
    let caps = detect_caps();
    unsafe {
        *VECTOR_CAPS.get() = caps;
    }
    caps
}

pub fn caps() -> VectorCaps {
    unsafe { *VECTOR_CAPS.get() }
}

pub fn context_mode() -> VectorContextMode {
    caps().context_mode
}

pub fn xsave_mask() -> u64 {
    caps().xsave_mask
}

pub fn render_proc_status() -> String {
    let caps = caps();
    let mut text = String::new();
    let _ = writeln!(text, "initialized {}", yes_no(caps.initialized));
    let _ = writeln!(text, "policy {}", caps.policy.as_str());
    let _ = writeln!(text, "context_mode {}", caps.context_mode.as_str());
    let _ = writeln!(text, "base_bits {:#018x}", caps.base_bits);
    let _ = writeln!(text, "ext_bits {:#018x}", caps.ext_bits);
    let _ = writeln!(text, "xsave_mask {:#018x}", caps.xsave_mask);

    let _ = writeln!(text, "feature_avx {}", yes_no(caps.has_base(VectorFeatureBits::AVX)));
    let _ = writeln!(text, "feature_avx2 {}", yes_no(caps.has_base(VectorFeatureBits::AVX2)));
    let _ = writeln!(
        text,
        "feature_avx512f {}",
        yes_no(caps.has_base(VectorFeatureBits::AVX512F))
    );
    let _ = writeln!(
        text,
        "feature_avx512bw {}",
        yes_no(caps.has_ext(VectorFeatureBits::AVX512BW))
    );
    let _ = writeln!(
        text,
        "feature_avx512dq {}",
        yes_no(caps.has_ext(VectorFeatureBits::AVX512DQ))
    );
    let _ = writeln!(
        text,
        "feature_avx512vl {}",
        yes_no(caps.has_ext(VectorFeatureBits::AVX512VL))
    );
    let _ = writeln!(text, "feature_neon {}", yes_no(caps.has_base(VectorFeatureBits::NEON)));
    let _ = writeln!(
        text,
        "feature_altivec {}",
        yes_no(caps.has_base(VectorFeatureBits::ALTIVEC))
    );
    let _ = writeln!(text, "feature_vsx {}", yes_no(caps.has_base(VectorFeatureBits::VSX)));
    text
}

pub fn append_cpuinfo_flags(text: &mut String) {
    let caps = caps();
    if caps.has_base(VectorFeatureBits::AVX) {
        append_flag(text, "avx");
    }
    if caps.has_base(VectorFeatureBits::AVX2) {
        append_flag(text, "avx2");
    }
    if caps.has_base(VectorFeatureBits::AVX512F) {
        append_flag(text, "avx512f");
    }
    if caps.has_ext(VectorFeatureBits::AVX512BW) {
        append_flag(text, "avx512bw");
    }
    if caps.has_ext(VectorFeatureBits::AVX512DQ) {
        append_flag(text, "avx512dq");
    }
    if caps.has_ext(VectorFeatureBits::AVX512VL) {
        append_flag(text, "avx512vl");
    }
    if caps.has_base(VectorFeatureBits::NEON) {
        append_flag(text, "neon");
    }
    if caps.has_base(VectorFeatureBits::ALTIVEC) {
        append_flag(text, "altivec");
    }
    if caps.has_base(VectorFeatureBits::VSX) {
        append_flag(text, "vsx");
    }
}

#[cfg(target_arch = "x86_64")]
fn detect_caps() -> VectorCaps {
    let leaf1 = arch::x86_64::cpuid(1);
    let leaf7_ebx = if arch::x86_64::max_basic_cpuid_leaf() >= 7 {
        arch::x86_64::cpuid_count(7, 0).ebx
    } else {
        0
    };
    let xcr0 = if (leaf1.ecx & (1 << 27)) != 0 { read_xcr0(0) } else { 0 };

    decode_x86_caps(leaf1.ecx, leaf1.edx, leaf7_ebx, xcr0)
}

#[cfg(target_arch = "x86_64")]
fn decode_x86_caps(leaf1_ecx: u32, leaf1_edx: u32, leaf7_ebx: u32, xcr0: u64) -> VectorCaps {
    const XCR0_XMM: u64 = 1 << 1;
    const XCR0_YMM: u64 = 1 << 2;
    const XCR0_OPMASK: u64 = 1 << 5;
    const XCR0_ZMM_HI256: u64 = 1 << 6;
    const XCR0_HI16_ZMM: u64 = 1 << 7;

    let mut caps = VectorCaps {
        initialized: true,
        ..VectorCaps::empty()
    };

    let has_xsave_instruction = (leaf1_ecx & (1 << 26)) != 0;
    let has_osxsave = (leaf1_ecx & (1 << 27)) != 0;
    let has_avx_hw = (leaf1_ecx & (1 << 28)) != 0;
    let has_fxsave = (leaf1_edx & (1 << 24)) != 0;
    let ymm_state_enabled = (xcr0 & (XCR0_XMM | XCR0_YMM)) == (XCR0_XMM | XCR0_YMM);
    let avx_usable = has_xsave_instruction && has_osxsave && has_avx_hw && ymm_state_enabled;

    if avx_usable {
        caps.base_bits |= VectorFeatureBits::AVX;
    }
    if avx_usable && (leaf7_ebx & (1 << 5)) != 0 {
        caps.base_bits |= VectorFeatureBits::AVX2;
    }

    let avx512_state_mask = XCR0_XMM | XCR0_YMM | XCR0_OPMASK | XCR0_ZMM_HI256 | XCR0_HI16_ZMM;
    let avx512_state_enabled = (xcr0 & avx512_state_mask) == avx512_state_mask;
    let avx512f = avx_usable && avx512_state_enabled && (leaf7_ebx & (1 << 16)) != 0;
    if avx512f {
        caps.base_bits |= VectorFeatureBits::AVX512F;
    }
    if avx512f && (leaf7_ebx & (1 << 30)) != 0 {
        caps.ext_bits |= VectorFeatureBits::AVX512BW;
    }
    if avx512f && (leaf7_ebx & (1 << 17)) != 0 {
        caps.ext_bits |= VectorFeatureBits::AVX512DQ;
    }
    if avx512f && (leaf7_ebx & (1 << 31)) != 0 {
        caps.ext_bits |= VectorFeatureBits::AVX512VL;
    }

    if has_xsave_instruction && has_osxsave && ymm_state_enabled {
        caps.context_mode = VectorContextMode::Xsave;
        caps.xsave_mask = if avx512_state_enabled {
            avx512_state_mask
        } else {
            XCR0_XMM | XCR0_YMM
        };
        caps.policy = VectorContextPolicy::Eager;
    } else if has_fxsave {
        caps.context_mode = VectorContextMode::Fxsave;
        caps.policy = VectorContextPolicy::Eager;
    } else {
        caps.context_mode = VectorContextMode::ScalarOnly;
        caps.policy = VectorContextPolicy::ScalarFallback;
    }

    caps
}

#[cfg(target_arch = "aarch64")]
fn detect_caps() -> VectorCaps {
    let mut caps = VectorCaps {
        initialized: true,
        ..VectorCaps::empty()
    };

    // AArch64 Linux/Unix-like environments effectively require ASIMD for general-purpose ABI use.
    caps.base_bits |= VectorFeatureBits::NEON;
    caps.context_mode = VectorContextMode::Aarch64FpSimd;
    caps.policy = VectorContextPolicy::Eager;
    caps
}

#[cfg(target_arch = "powerpc64")]
fn detect_caps() -> VectorCaps {
    let mut caps = VectorCaps {
        initialized: true,
        ..VectorCaps::empty()
    };

    if cfg!(target_feature = "altivec") {
        caps.base_bits |= VectorFeatureBits::ALTIVEC;
    }
    if cfg!(target_feature = "vsx") {
        caps.base_bits |= VectorFeatureBits::VSX;
        caps.context_mode = VectorContextMode::Ppc64Vsx;
        caps.policy = VectorContextPolicy::Eager;
    } else {
        caps.context_mode = VectorContextMode::ScalarOnly;
        caps.policy = VectorContextPolicy::ScalarFallback;
    }

    caps
}

#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "powerpc64"
)))]
fn detect_caps() -> VectorCaps {
    VectorCaps {
        initialized: true,
        ..VectorCaps::empty()
    }
}

#[cfg(target_arch = "x86_64")]
fn read_xcr0(index: u32) -> u64 {
    let eax: u32;
    let edx: u32;
    unsafe {
        asm!(
            "xgetbv",
            in("ecx") index,
            out("eax") eax,
            out("edx") edx,
            options(nomem, nostack, preserves_flags),
        );
    }
    ((edx as u64) << 32) | eax as u64
}

fn append_flag(text: &mut String, flag: &str) {
    if !text.is_empty() {
        text.push(' ');
    }
    text.push_str(flag);
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
