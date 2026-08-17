// TCOL / HPL (HXNU Public License)
// TCOL v1.1 / HPL (HXNU Public License)
// This file is strictly governed by the Tile Conservative Open License (TCOL v1.1).

//! # RK3588S Memory-Mapped Register Map & MMIO Helper Functions
//!
//! This module defines physical base addresses, register offsets, bit fields,
//! and low-level volatile Memory-Mapped I/O (MMIO) utility functions for the
//! Rockchip RK3588S SoC (Orange Pi 5 8GB).
//!
//! ## High-Word Write Enable Mask Mechanism
//! Rockchip GRF (General Register File) and CRU (Clock & Reset Unit) registers
//! enforce a mandatory high-word write-enable mask on 32-bit registers:
//! - Bits `[15:0]` contain the operational control bits.
//! - Bits `[31:16]` contain write-enable mask bits corresponding to `[15:0]`.
//! - To modify bit $i$ ($0 \le i \le 15$), bit $(i + 16)$ MUST be set to `1`.
//!   Writing to bit $i$ without setting bit $(i + 16)$ to `1` causes the hardware to
//!   silently ignore the write to bit $i$.

#![allow(dead_code)]

// ============================================================================
// 1. PHYSICAL BASE ADDRESSES
// ============================================================================

/// System General Register File (SYS_GRF) physical base address (`0xFD580000`)
pub const SYS_GRF_BASE: usize = 0xFD58_0000;

/// Peripherals General Register File (PHP_GRF) physical base address (`0xFD5B0000`)
pub const PHP_GRF_BASE: usize = 0xFD5B_0000;

/// PCIe 3.0 PHY General Register File (PCIE30_PHY_GRF) physical base address (`0xFD5B8000`)
pub const PCIE30_PHY_GRF_BASE: usize = 0xFD5B_8000;

/// CombPHY 0 General Register File physical base address (`0xFD5C0000`)
pub const PIPE_PHY0_GRF_BASE: usize = 0xFD5C_0000;

/// CombPHY 1 General Register File physical base address (`0xFD5C8000`)
pub const PIPE_PHY1_GRF_BASE: usize = 0xFD5C_8000;

/// CombPHY 2 General Register File physical base address (`0xFD5D0000`)
pub const PIPE_PHY2_GRF_BASE: usize = 0xFD5D_0000;

/// Clock & Reset Unit (CRU) physical base address (`0xFD7C0000`)
pub const CRU_BASE: usize = 0xFD7C_0000;

/// Power Management Unit (PMU) physical base address (`0xFD8D0000`)
pub const PMU_BASE: usize = 0xFD8D_0000;

/// DesignWare PCIe 3.0 x4 Controller DBI physical base address (`0xFE150000`)
pub const PCIE30X4_DBI_BASE: usize = 0xFE15_0000;
pub const PCIE30X4_BASE: usize = PCIE30X4_DBI_BASE;

/// DesignWare PCIe 2.0 x1 Controller 0 DBI physical base address (`0xFE170000`)
pub const PCIE2X1L0_DBI_BASE: usize = 0xFE17_0000;

/// DesignWare PCIe 2.0 x1 Controller 1 DBI physical base address (`0xFE180000`)
pub const PCIE2X1L1_DBI_BASE: usize = 0xFE18_0000;

/// Debug UART2 MMIO physical base address (`0xFEB50000`)
pub const RK3588_UART2_BASE: usize = 0xFEB5_0000;

/// GICv3 Distributor (GICD) physical base address (`0xFE600000`)
pub const GICD_BASE: usize = 0xFE60_0000;

/// GICv3 Redistributor (GICR) physical base address (`0xFE660000`)
pub const GICR_BASE: usize = 0xFE66_0000;

/// GICv3 Distributor Control Register physical address (`0xFE600000`)
pub const GICD_CTLR: usize = GICD_BASE + 0x0000;

pub mod gicd {
    use super::GICD_BASE;

    /// GICD Control Register offset (`0x0000`)
    pub const CTLR_OFFSET: usize = 0x0000;
    /// Full physical address of `GICD_CTLR` (`0xFE600000`)
    pub const GICD_CTLR: usize = GICD_BASE + CTLR_OFFSET;
}


// ============================================================================
// 2. REGISTER OFFSETS AND BIT DEFINITIONS
// ============================================================================

/// SYS_GRF Register Offsets and Bit Definitions
pub mod sys_grf {
    use super::SYS_GRF_BASE;

    /// System GRF SoC Control Register 0 offset (`0x0300`)
    pub const SOC_CON0_OFFSET: usize = 0x0300;
    /// Full physical address of `SYS_GRF_SOC_CON0` (`0xFD580300`)
    pub const SYS_GRF_SOC_CON0: usize = SYS_GRF_BASE + SOC_CON0_OFFSET;

    /// Bit 4: PCIE_EP_MODE_FORCE (1 = Force top-level SoC interconnect into EP mode)
    pub const PCIE_EP_MODE_FORCE_BIT: u8 = 4;
    pub const PCIE_EP_MODE_FORCE_MASK: u16 = 1 << 4;
    pub const PCIE_EP_MODE_FORCE_ENABLE: u16 = 1 << 4;
    pub const PCIE_EP_MODE_FORCE_DISABLE: u16 = 0 << 4;

    /// Combined write-enable masked value for forcing top-level EP mode (`(1<<20) | (1<<4)`)
    pub const SYS_GRF_SOC_CON0_EP_FORCE: u32 = (1 << 20) | (1 << 4);
}

/// PHP_GRF Register Offsets and Bit Definitions
pub mod php_grf {
    use super::PHP_GRF_BASE;

    /// Peripherals GRF Control Register 0 offset (`0x0000`)
    pub const CON0_OFFSET: usize = 0x0000;
    /// Full physical address of `PHP_GRF_CON0` (`0xFD5B0000`)
    pub const PHP_GRF_CON0: usize = PHP_GRF_BASE + CON0_OFFSET;

    /// Peripherals GRF Control Register 1 offset (`0x0004`)
    pub const CON1_OFFSET: usize = 0x0004;
    /// Full physical address of `PHP_GRF_CON1` (`0xFD5B0004`)
    pub const PHP_GRF_CON1: usize = PHP_GRF_BASE + CON1_OFFSET;

    /// CON0 Bit 0: PCIE30X4_DEVICE_TYPE (0 = Root Complex RC, 1 = Endpoint EP)
    pub const PCIE30X4_DEVICE_TYPE_BIT: u8 = 0;
    pub const PCIE30X4_DEVICE_TYPE_MASK: u16 = 1 << 0;
    pub const PCIE30X4_DEVICE_TYPE_RC: u16 = 0 << 0;
    pub const PCIE30X4_DEVICE_TYPE_EP: u16 = 1 << 0;

    /// CON0 Bit 1: PCIE30X4_LINK_MODE (0 = x4 aggregation mode, 1 = x2x2 bifurcation mode)
    pub const PCIE30X4_LINK_MODE_BIT: u8 = 1;
    pub const PCIE30X4_LINK_MODE_MASK: u16 = 1 << 1;
    pub const PCIE30X4_LINK_MODE_X4: u16 = 0 << 1;
    pub const PCIE30X4_LINK_MODE_X2X2: u16 = 1 << 1;

    /// CON0 Bits [3:2]: PCIE30X4_LANE_NUM
    pub const PCIE30X4_LANE_NUM_MASK: u16 = 3 << 2;

    /// Combined write-enable masked values
    pub const PHP_GRF_CON0_EP_MODE: u32 = (1 << 16) | (1 << 0);
    pub const PHP_GRF_CON0_LINK_X4: u32 = (1 << 17) | (0 << 1);
}

/// PCIE30_PHY_GRF Register Offsets and Bit Definitions
pub mod pcie30_phy_grf {
    use super::PCIE30_PHY_GRF_BASE;

    /// PCIe 3.0 PHY Control Register 0 offset (`0x0000`)
    pub const CON0_OFFSET: usize = 0x0000;
    /// Full physical address of `PCIE30_PHY_GRF_CON0` (`0xFD5B8000`)
    pub const PCIE30_PHY_GRF_CON0: usize = PCIE30_PHY_GRF_BASE + CON0_OFFSET;

    /// PCIe 3.0 PHY Control Register 1 offset (`0x0004`)
    pub const CON1_OFFSET: usize = 0x0004;
    /// Full physical address of `PCIE30_PHY_GRF_CON1` (`0xFD5B8004`)
    pub const PCIE30_PHY_GRF_CON1: usize = PCIE30_PHY_GRF_BASE + CON1_OFFSET;

    /// PCIe 3.0 PHY Status Register 0 offset (`0x0080`, Read-Only)
    pub const STATUS0_OFFSET: usize = 0x0080;
    /// Full physical address of `PCIE30_PHY_GRF_STATUS0` (`0xFD5B8080`)
    pub const PCIE30_PHY_GRF_STATUS0: usize = PCIE30_PHY_GRF_BASE + STATUS0_OFFSET;

    /// CON0 Bit 0: PHY_MODE_SEL (0 = RC PHY, 1 = EP PHY)
    pub const PHY_MODE_SEL_BIT: u8 = 0;
    pub const PHY_MODE_SEL_MASK: u16 = 1 << 0;
    pub const PHY_MODE_SEL_RC: u16 = 0 << 0;
    pub const PHY_MODE_SEL_EP: u16 = 1 << 0;

    /// CON0 Bit 1: REFCLK_SEL (0 = External Host 100MHz clock, 1 = Internal SoC PLL)
    pub const REFCLK_SEL_BIT: u8 = 1;
    pub const REFCLK_SEL_MASK: u16 = 1 << 1;
    pub const REFCLK_SEL_EXT: u16 = 0 << 1;
    pub const REFCLK_SEL_INT: u16 = 1 << 1;

    /// CON0 Bit 4: PHY_RESET_N (0 = assert PHY reset, 1 = de-assert/release PHY reset)
    pub const PHY_RESET_N_BIT: u8 = 4;
    pub const PHY_RESET_N_MASK: u16 = 1 << 4;
    pub const PHY_RESET_N_ASSERT: u16 = 0 << 4;
    pub const PHY_RESET_N_RELEASE: u16 = 1 << 4;

    /// STATUS0 Bit 0: PHY_PLL_LOCK (1 = PHY PLL locked at 100MHz)
    pub const STATUS0_PHY_PLL_LOCK: u32 = 1 << 0;
    /// STATUS0 Bit 1: PHY_SRAM_INIT_DONE (1 = PHY SRAM calibration done)
    pub const STATUS0_PHY_SRAM_INIT_DONE: u32 = 1 << 1;

    /// Combined write-enable masked values
    pub const PCIE30_PHY_GRF_CON0_EP_MODE: u32 = (1 << 16) | (1 << 0);
    pub const PCIE30_PHY_GRF_CON0_REFCLK_EXT: u32 = (1 << 17) | (0 << 1);
    pub const PCIE30_PHY_GRF_CON0_PHY_RST_DEASSERT: u32 = (1 << 20) | (1 << 4);
}

/// CRU (Clock & Reset Unit) Register Offsets and Bit Definitions
pub mod cru {
    use super::CRU_BASE;

    /// Clock Gate Control Register 28 offset (`0x0370`)
    pub const CLKGATE_CON28_OFFSET: usize = 0x0370;
    pub const CRU_CLKGATE_CON28: usize = CRU_BASE + CLKGATE_CON28_OFFSET;

    /// Clock Gate Control Register 29 offset (`0x0374`)
    pub const CLKGATE_CON29_OFFSET: usize = 0x0374;
    pub const CRU_CLKGATE_CON29: usize = CRU_BASE + CLKGATE_CON29_OFFSET;

    /// Soft Reset Control Register 28 offset (`0x0A70`)
    pub const SOFTRST_CON28_OFFSET: usize = 0x0A70;
    pub const CRU_SOFTRST_CON28: usize = CRU_BASE + SOFTRST_CON28_OFFSET;

    /// CLKGATE_CON28 Bits: 0 = ungate (enable clock), 1 = gate (disable clock)
    pub const CLK_PCIE30X4_MSTR_BIT: u16 = 1 << 0;
    pub const CLK_PCIE30X4_SLV_BIT: u16 = 1 << 1;
    pub const CLK_PCIE30X4_DBI_BIT: u16 = 1 << 2;
    pub const CLKGATE_CON28_ALL_MASK: u16 = 0x0007;

    /// CLKGATE_CON29 Bits
    pub const CLK_PCIE30_PHY_REF_BIT: u16 = 1 << 4;
    pub const PCLK_PCIE30_BIT: u16 = 1 << 5;
    pub const CLKGATE_CON29_ALL_MASK: u16 = 0x0030;

    /// SOFTRST_CON28 Bits: 1 = assert reset, 0 = release reset
    pub const RST_PCIE30X4_ARST_BIT: u16 = 1 << 0;
    pub const RST_PCIE30X4_URST_BIT: u16 = 1 << 1;
    pub const RST_PCIE30X4_PRST_BIT: u16 = 1 << 2;
    pub const RST_PCIE30_PHY_BIT: u16 = 1 << 3;

    pub const SOFTRST_ALL_MASK: u16 = 0x000F;
    pub const SOFTRST_PHY_MASK: u16 = 0x0008;
    pub const SOFTRST_CTRL_MASK: u16 = 0x0007;

    /// Pre-calculated masked values
    pub const CRU_CLKGATE_CON28_PCIE_UNGATE: u32 = 0x0007_0000;
    pub const CRU_CLKGATE_CON29_PCIE_UNGATE: u32 = 0x0030_0000;
    pub const CRU_SOFTRST_CON28_ASSERT_ALL: u32 = 0x000F_000F;
    pub const CRU_SOFTRST_CON28_DEASSERT_PHY: u32 = 0x0008_0000;
    pub const CRU_SOFTRST_CON28_DEASSERT_ALL: u32 = 0x000F_0000;
}

/// PMU (Power Management Unit) Register Offsets and Bit Definitions
pub mod pmu {
    use super::PMU_BASE;

    /// Power Domain Control Register offset (`0x0000`)
    pub const PD_CON_OFFSET: usize = 0x0000;
    pub const PMU_PD_CON: usize = PMU_BASE + PD_CON_OFFSET;

    /// Power Domain Status Register offset (`0x0080`, Read-Only)
    pub const POWER_ST_OFFSET: usize = 0x0080;
    pub const PMU_POWER_ST: usize = PMU_BASE + POWER_ST_OFFSET;

    /// PD_CON Bit 4: PD_PCIE Control (0 = Power ON, 1 = Power OFF)
    pub const PD_PCIE_CON_MASK: u16 = 1 << 4;
    pub const PD_PCIE_CON_PWR_ON: u16 = 0 << 4;
    pub const PD_PCIE_CON_PWR_OFF: u16 = 1 << 4;

    /// POWER_ST Bit 4: PD_PCIE Status (0 = Active & Stable, 1 = Off)
    pub const PD_PCIE_PWR_ST_ACTIVE: u32 = 0 << 4;
    pub const PD_PCIE_PWR_ST_MASK: u32 = 1 << 4;

    /// Pre-calculated power-on masked value (`(1<<20) | (0<<4)`)
    pub const PMU_PD_CON_PCIE_ENABLE: u32 = (1 << 20) | (0 << 4);
    pub const PMU_POWER_ST_PD_PCIE_MASK: u32 = 1 << 4;
}

/// DesignWare PCIe Controller DBI Register Offsets
pub mod pcie_dbi {
    pub const PCI_VENDOR_DEVICE_ID: usize = 0x0000;
    pub const PCI_COMMAND_STATUS: usize = 0x0004;
    pub const PCI_CLASS_REVISION: usize = 0x0008;
    pub const PCI_BAR0: usize = 0x0010;
    pub const PCI_BAR1: usize = 0x0014;
    pub const PCI_BAR2: usize = 0x0018;
    pub const PCI_BAR3: usize = 0x001C;
    pub const PCI_BAR4: usize = 0x0020;
    pub const PCI_BAR5: usize = 0x0024;
    pub const PCI_SUBSYSTEM_ID: usize = 0x002C;

    pub const PORT_LINK_CTRL_OFF: usize = 0x0710;
    pub const GEN2_CTRL_OFF: usize = 0x080C;
    pub const MISC_CONTROL_1_OFF: usize = 0x08BC;

    /// MISC_CONTROL_1_OFF Bit 0: DBI_RO_WR_EN (1 = Enable writing to RO fields in DBI)
    pub const DBI_RO_WR_EN: u32 = 1 << 0;
}

// ============================================================================
// 3. WRITE ENABLE MASK HELPERS & MMIO ABSTRACTIONS
// ============================================================================

/// Calculates the 32-bit Rockchip GRF/CRU write-enable masked value.
///
/// In Rockchip GRF and CRU registers, bits `[31:16]` act as write-enable masks
/// for operational bits `[15:0]`.
#[inline(always)]
pub const fn write_enable_mask(mask: u16, val: u16) -> u32 {
    ((mask as u32) << 16) | ((val & mask) as u32)
}

/// Alias for `write_enable_mask` to support `high_word_mask` name requirement.
#[inline(always)]
pub const fn high_word_mask(mask: u16, val: u16) -> u32 {
    write_enable_mask(mask, val)
}

/// Calculates write-enable mask for a single bit index (0..15).
#[inline(always)]
pub const fn write_enable_bit(bit: u8, val: bool) -> u32 {
    let mask = 1u16 << bit;
    let val_bits = if val { mask } else { 0 };
    write_enable_mask(mask, val_bits)
}

/// Low-level raw volatile read from 32-bit MMIO address.
///
/// # Safety
/// `addr` must be a valid, 32-bit aligned MMIO memory address.
#[inline(always)]
pub unsafe fn read_volatile(addr: usize) -> u32 {
    core::ptr::read_volatile(addr as *const u32)
}

/// Low-level raw volatile write to 32-bit MMIO address.
///
/// # Safety
/// `addr` must be a valid, 32-bit aligned MMIO memory address.
#[inline(always)]
pub unsafe fn write_volatile(addr: usize, val: u32) {
    core::ptr::write_volatile(addr as *mut u32, val);
}

/// Perform volatile read of 32-bit register at `base + offset`.
///
/// # Safety
/// The address `(base + offset)` must be a valid, aligned MMIO memory address.
#[inline(always)]
pub unsafe fn read_reg(base: usize, offset: usize) -> u32 {
    read_volatile(base + offset)
}

/// Perform volatile write of 32-bit register at `base + offset`.
///
/// # Safety
/// The address `(base + offset)` must be a valid, aligned MMIO memory address.
#[inline(always)]
pub unsafe fn write_reg(base: usize, offset: usize, val: u32) {
    write_volatile(base + offset, val);
}

/// Perform volatile write with high-word write-enable mask to GRF/CRU register at `base + offset`.
///
/// # Safety
/// The address `(base + offset)` must be a valid, aligned MMIO memory address.
#[inline(always)]
pub unsafe fn write_reg_masked(base: usize, offset: usize, mask: u16, val: u16) {
    let masked_val = high_word_mask(mask, val);
    write_reg(base, offset, masked_val);
}

/// Perform a write to a Rockchip GRF/CRU register address with high-word write enable mask for a single bit.
///
/// # Safety
/// `addr` must be a valid, aligned MMIO address.
#[inline(always)]
pub unsafe fn grf_write_bit(addr: usize, bit: u8, val: bool) {
    let masked_val = write_enable_bit(bit, val);
    write_volatile(addr, masked_val);
}

/// Perform a write to a Rockchip GRF/CRU register address with a multi-bit high-word enable mask.
///
/// # Safety
/// `addr` must be a valid, aligned MMIO address.
#[inline(always)]
pub unsafe fn grf_write_mask(addr: usize, mask: u16, val: u16) {
    let masked_val = high_word_mask(mask, val);
    write_volatile(addr, masked_val);
}

/// Read-Modify-Write helper for standard non-GRF registers (e.g. PMU, DBI).
///
/// # Safety
/// `addr` must be a valid, aligned MMIO memory address.
#[inline(always)]
pub unsafe fn modify_volatile<F>(addr: usize, f: F)
where
    F: FnOnce(u32) -> u32,
{
    let val = read_volatile(addr);
    let new_val = f(val);
    write_volatile(addr, new_val);
}

/// Perform volatile read-modify-write on register at `base + offset`.
///
/// # Safety
/// The address `(base + offset)` must be a valid, aligned MMIO memory address.
#[inline(always)]
pub unsafe fn modify_reg<F>(base: usize, offset: usize, f: F)
where
    F: FnOnce(u32) -> u32,
{
    modify_volatile(base + offset, f);
}
