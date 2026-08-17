// TCOL v1.1 / HPL (HXNU Public License)
// This file is strictly governed by the Tile Conservative Open License (TCOL v1.1).

//! # Rockchip RK3588S PCIe 3.0 Endpoint (EP) Controller & PHY Initialization
//!
//! This module performs bare-metal hardware configuration to force the Rockchip RK3588S
//! PCIe 3.0 x4 controller and PHY into Endpoint (EP) mode.
//!
//! ## Overview of Register Bit-Flips & High-Word Write Masks
//! The Rockchip GRF (General Register File) and CRU (Clock & Reset Unit) hardware registers
//! implement a high-word write-enable mask pattern:
//! - Lower 16 bits `[15:0]` store operational control configuration values.
//! - Upper 16 bits `[31:16]` store write-enable masks corresponding to `[15:0]`.
//! - For any write to bit $N$ ($0 \le N \le 15$), bit $(N + 16)$ must be set to `1`.
//!   Formula: `val_32 = (mask_16 << 16) | (val_16 & mask_16)`.
//!
//! ## Execution Sequence
//! 1. **Top-Level Wrapper Bit-Flip (`SYS_GRF_SOC_CON0` @ `0xFD580300`):**
//!    Forces top-level SoC interconnect into Endpoint mode by setting Bit 4 (`PCIE_EP_MODE_FORCE`).
//!    Value written: `(1 << 20) | (1 << 4)` = `0x00100010`.
//! 2. **Controller EP Mode Bit-Flip (`PHP_GRF_CON0` @ `0xFD5B0000`):**
//!    Forces DesignWare PCIe 3.0 x4 controller device type to Endpoint mode by setting Bit 0 (`PCIE30X4_DEVICE_TYPE`).
//!    Value written: `(1 << 16) | (1 << 0)` = `0x00010001`.
//! 3. **PHY EP & Reference Clock Bit-Flips (`PCIE30_PHY_GRF_CON0` @ `0xFD5B8000`):**
//!    - Forces PHY into EP mode by setting Bit 0 (`PHY_MODE_SEL` = 1). Value written: `(1 << 16) | (1 << 0)` = `0x00010001`.
//!    - Selects external host 100MHz bus reference clock by clearing Bit 1 (`REFCLK_SEL` = 0). Value written: `(1 << 17) | (0 << 1)` = `0x00020000`.
//!    - Combined high-word write-enable value: `(0x0003 << 16) | (0x0001)` = `0x00030001`.
//! 4. **Power Domain Enable (`PMU_PD_CON` @ `0xFD8D0000`):**
//!    Enables `PD_PCIE` power domain by clearing Bit 4 (`PD_PCIE_CON` = 0).
//!    Value written: `(1 << 20) | (0 << 4)` = `0x00100000`.
//!    Polls `PMU_POWER_ST` (`0xFD8D0080` Bit 4) until power status is active (0).
//! 5. **Clock Ungating (`CRU_CLKGATE_CON28` / `CON29` @ `0xFD7C0370` / `0xFD7C0374`):**
//!    Ungates master, slave, DBI, PHY reference, and APB clocks by clearing gate bits (0 = ungate).
//!    - `CRU_CLKGATE_CON28` Bits `[2:0]`: `(0x0007 << 16) | 0x0000` = `0x00070000`.
//!    - `CRU_CLKGATE_CON29` Bits `[5:4]`: `(0x0030 << 16) | 0x0000` = `0x00300000`.
//! 6. **Reset De-assertion (`CRU_SOFTRST_CON28` @ `0xFD7C0A70` & `PCIE30_PHY_GRF_CON0` Bit 4):**
//!    Asserts and then de-asserts soft resets to bring the PCIe controller and PHY out of reset.
//!    - Soft reset assert: `(0x000F << 16) | 0x000F` = `0x000F000F`.
//!    - Soft reset de-assert PHY: `(0x0008 << 16) | 0x0000` = `0x00080000`.
//!    - Soft reset de-assert Controller: `(0x0007 << 16) | 0x0000` = `0x00070000`.
//!    - Release PHY internal reset in `PCIE30_PHY_GRF_CON0` Bit 4 (`PHY_RESET_N` = 1): `(1 << 20) | (1 << 4)` = `0x00100010`.
//! 7. **PHY PLL Lock Polling (`PCIE30_PHY_GRF_STATUS0` @ `0xFD5B8080`):**
//!    Polls Status Bit 0 (`PHY_PLL_LOCK`) until the PHY PLL locks onto the host 100MHz reference clock.

use crate::uart_print;
use crate::rk3588_regs::{
    cru, pcie30_phy_grf, php_grf, pmu, sys_grf, write_enable_mask, write_reg, read_reg,
};

/// High-level function to initialize RK3588S PCIe 3.0 Endpoint Mode.
///
/// Configures top-level system GRF, peripheral GRF, PHY GRF, PMU power domains,
/// CRU clocks, reset signals, and polls for PHY PLL lock.
pub fn init_pcie_endpoint() {
    uart_print("[G-BOOT] Starting PCIe 3.0 Endpoint Mode Bit-Flip Sequence...\n");

    unsafe {
        // =====================================================================
        // STEP 1: Force Top-Level Interconnect Wrapper into Endpoint Mode
        // =====================================================================
        // Address: SYS_GRF_SOC_CON0 (0xFD580300)
        // Bit 4: PCIE_EP_MODE_FORCE = 1
        // High-word write mask: (1 << (4 + 16)) | (1 << 4) = 0x00100010
        // Hardware effect: Directs internal SoC bus crossbar routing so PCIe 3.0
        // signals operate in Endpoint target mode rather than Root Complex master mode.
        uart_print("[G-BOOT] STEP 1: Writing SYS_GRF_SOC_CON0 @ 0xFD580300 (Bit 4 = 1 -> 0x00100010)...\n");
        let sys_grf_val = write_enable_mask(
            sys_grf::PCIE_EP_MODE_FORCE_MASK,
            sys_grf::PCIE_EP_MODE_FORCE_ENABLE,
        );
        write_reg(sys_grf::SYS_GRF_SOC_CON0, 0, sys_grf_val);

        // =====================================================================
        // STEP 2: Force PCIe 3.0 x4 Controller into Endpoint (EP) Mode
        // =====================================================================
        // Address: PHP_GRF_CON0 (0xFD5B0000)
        // Bit 0: PCIE30X4_DEVICE_TYPE = 1 (0 = Root Complex, 1 = Endpoint)
        // High-word write mask: (1 << (0 + 16)) | (1 << 0) = 0x00010001
        // Hardware effect: Configures DesignWare PCIe controller IP hardware registers
        // to respond to Host memory space reads/writes and configuration cycles.
        uart_print("[G-BOOT] STEP 2: Writing PHP_GRF_CON0 @ 0xFD5B0000 (Bit 0 = 1 -> 0x00010001)...\n");
        let php_grf_val = write_enable_mask(
            php_grf::PCIE30X4_DEVICE_TYPE_MASK,
            php_grf::PCIE30X4_DEVICE_TYPE_EP,
        );
        write_reg(php_grf::PHP_GRF_CON0, 0, php_grf_val);

        // =====================================================================
        // STEP 3: Force PCIe 3.0 PHY into Endpoint Mode & Select External Refclk
        // =====================================================================
        // Address: PCIE30_PHY_GRF_CON0 (0xFD5B8000)
        // Bit 0: PHY_MODE_SEL = 1 (1 = EP PHY mode) -> (1 << 16) | (1 << 0) = 0x00010001
        // Bit 1: REFCLK_SEL = 0 (0 = External Host 100MHz refclk) -> (1 << 17) | (0 << 1) = 0x00020000
        // Combined Mask: (0x0003 << 16) | (0x0001) = 0x00030001
        // Hardware effect: Configures high-speed SerDes analog PHY for Endpoint termination
        // and routes external 100MHz PCIe reference clock from PCIe edge connector/M.2 slot.
        uart_print("[G-BOOT] STEP 3: Writing PCIE30_PHY_GRF_CON0 @ 0xFD5B8000 (Bit 0 = 1, Bit 1 = 0 -> 0x00030001)...\n");
        let phy_grf_mask = pcie30_phy_grf::PHY_MODE_SEL_MASK | pcie30_phy_grf::REFCLK_SEL_MASK;
        let phy_grf_val_bits = pcie30_phy_grf::PHY_MODE_SEL_EP | pcie30_phy_grf::REFCLK_SEL_EXT;
        let phy_grf_val = write_enable_mask(phy_grf_mask, phy_grf_val_bits);
        write_reg(pcie30_phy_grf::PCIE30_PHY_GRF_CON0, 0, phy_grf_val);

        // =====================================================================
        // STEP 4: Power Domain Activation (PD_PCIE Power ON)
        // =====================================================================
        // Address: PMU_PD_CON (0xFD8D0000)
        // Bit 4: PD_PCIE_CON = 0 (0 = Power ON, 1 = Power OFF)
        // High-word write mask: (1 << (4 + 16)) | (0 << 4) = 0x00100000
        // Hardware effect: Powers on the physical power island supplying voltage to the
        // PCIe 3.0 controller logic and high-speed analog SerDes PHY.
        uart_print("[G-BOOT] STEP 4: Enabling PD_PCIE Power Domain via PMU_PD_CON @ 0xFD8D0000...\n");
        let pmu_val = write_enable_mask(pmu::PD_PCIE_CON_MASK, pmu::PD_PCIE_CON_PWR_ON);
        write_reg(pmu::PMU_PD_CON, 0, pmu_val);

        // Bounded poll on PMU_POWER_ST (0xFD8D0080) Bit 4 until stable active status (0)
        uart_print("[G-BOOT] Polling PMU_POWER_ST @ 0xFD8D0080 Bit 4 for PD_PCIE active state...\n");
        let mut pmu_timeout = 100_000;
        while (read_reg(pmu::PMU_POWER_ST, 0) & pmu::PMU_POWER_ST_PD_PCIE_MASK) != 0 {
            pmu_timeout -= 1;
            if pmu_timeout == 0 {
                uart_print("[G-BOOT] WARNING: PMU PD_PCIE power domain polling timed out!\n");
                break;
            }
            core::hint::spin_loop();
        }
        uart_print("[G-BOOT] PD_PCIE Power Domain active and stable.\n");

        // =====================================================================
        // STEP 5: Clock Ungating (Master, Slave, DBI, PHY Refclk, APB)
        // =====================================================================
        // CRU_CLKGATE_CON28 (0xFD7C0370): Bit 0 (Master), Bit 1 (Slave), Bit 2 (DBI)
        // In Rockchip CRU, writing 0 to clock gate bit un-gates (enables) the clock.
        // High-word mask: (0x0007 << 16) | 0x0000 = 0x00070000
        uart_print("[G-BOOT] STEP 5a: Ungating PCIe Controller Clocks via CRU_CLKGATE_CON28 @ 0xFD7C0370...\n");
        let clkgate28_val = write_enable_mask(cru::CLKGATE_CON28_ALL_MASK, 0x0000);
        write_reg(cru::CRU_CLKGATE_CON28, 0, clkgate28_val);

        // CRU_CLKGATE_CON29 (0xFD7C0374): Bit 4 (PHY Refclk), Bit 5 (APB pclk)
        // High-word mask: (0x0030 << 16) | 0x0000 = 0x00300000
        uart_print("[G-BOOT] STEP 5b: Ungating PHY Refclk & APB Clocks via CRU_CLKGATE_CON29 @ 0xFD7C0374...\n");
        let clkgate29_val = write_enable_mask(cru::CLKGATE_CON29_ALL_MASK, 0x0000);
        write_reg(cru::CRU_CLKGATE_CON29, 0, clkgate29_val);

        // =====================================================================
        // STEP 6: Reset Sequence (Soft Reset De-assertion)
        // =====================================================================
        // Address: CRU_SOFTRST_CON28 (0xFD7C0A70)
        // In Rockchip CRU soft reset registers, 1 = assert reset, 0 = release/de-assert reset.
        // Step 6a: Assert resets across controller & PHY logic (0x000F000F)
        uart_print("[G-BOOT] STEP 6a: Asserting soft resets via CRU_SOFTRST_CON28 @ 0xFD7C0A70...\n");
        let rst_assert_val = write_enable_mask(cru::SOFTRST_ALL_MASK, cru::SOFTRST_ALL_MASK);
        write_reg(cru::CRU_SOFTRST_CON28, 0, rst_assert_val);

        // Small spin delay for reset propagation
        for _ in 0..1_000 {
            core::hint::spin_loop();
        }

        // Step 6b: Release/de-assert PHY soft reset (Bit 3 = 0) -> (0x0008 << 16) | 0x0000 = 0x00080000
        uart_print("[G-BOOT] STEP 6b: De-asserting PHY soft reset via CRU_SOFTRST_CON28...\n");
        let rst_deassert_phy = write_enable_mask(cru::SOFTRST_PHY_MASK, 0x0000);
        write_reg(cru::CRU_SOFTRST_CON28, 0, rst_deassert_phy);

        // Step 6c: Release PHY internal reset in PCIE30_PHY_GRF_CON0 Bit 4 (PHY_RESET_N = 1)
        // High-word mask: (1 << 20) | (1 << 4) = 0x00100010
        uart_print("[G-BOOT] STEP 6c: Releasing PHY internal reset via PCIE30_PHY_GRF_CON0 @ 0xFD5B8000 Bit 4...\n");
        let phy_rst_release_val = write_enable_mask(
            pcie30_phy_grf::PHY_RESET_N_MASK,
            pcie30_phy_grf::PHY_RESET_N_RELEASE,
        );
        write_reg(pcie30_phy_grf::PCIE30_PHY_GRF_CON0, 0, phy_rst_release_val);

        // Step 6d: Release controller soft resets (Bits [2:0] = 0) -> (0x0007 << 16) | 0x0000 = 0x00070000
        uart_print("[G-BOOT] STEP 6d: De-asserting PCIe controller soft resets via CRU_SOFTRST_CON28...\n");
        let rst_deassert_ctrl = write_enable_mask(cru::SOFTRST_CTRL_MASK, 0x0000);
        write_reg(cru::CRU_SOFTRST_CON28, 0, rst_deassert_ctrl);

        // =====================================================================
        // STEP 7: Poll PCIe 3.0 PHY PLL Lock Status
        // =====================================================================
        // Address: PCIE30_PHY_GRF_STATUS0 (0xFD5B8080)
        // Bit 0: PHY_PLL_LOCK = 1 indicates analog SerDes PLL has locked onto the 100MHz reference clock.
        uart_print("[G-BOOT] STEP 7: Polling PCIE30_PHY_GRF_STATUS0 @ 0xFD5B8080 Bit 0 for PHY_PLL_LOCK...\n");
        let mut pll_lock_timeout = 10_000_000;
        let mut pll_locked = false;

        while pll_lock_timeout > 0 {
            let status = read_reg(pcie30_phy_grf::PCIE30_PHY_GRF_STATUS0, 0);
            if (status & pcie30_phy_grf::STATUS0_PHY_PLL_LOCK) != 0 {
                pll_locked = true;
                break;
            }
            pll_lock_timeout -= 1;
            core::hint::spin_loop();
        }

        if pll_locked {
            uart_print("[G-BOOT] SUCCESS: PCIe 3.0 PHY PLL Locked (PHY_PLL_LOCK = 1).\n");
        } else {
            uart_print("[G-BOOT] WARNING: PCIe 3.0 PHY PLL Lock polling timed out! (Host refclk present?)\n");
        }
    }

    uart_print("[G-BOOT] PCIe 3.0 Endpoint Initialization Sequence Complete.\n");
}
