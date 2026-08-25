// THOL (Turkish Hybrid Open License)
// This file is strictly governed by the Turkish Hybrid Open License (THOL).
//! SXRC core payload definitions.
//!
//! This module provides the structural definitions for processing compressed
//! HEX2 and HEX4 SXRC payloads within the HXNU kernel.

/// Represents a compressed HEX2 SXRC payload.
/// 
/// The HEX2 format is designed for lightweight, low-latency offload tasks
/// suitable for the Supernova driver.
#[derive(Debug, Clone)]
pub struct Hex2Payload {
    /// The actual compressed data payload.
    pub data: [u8; 32],
    /// The size of the valid data in the payload.
    pub size: usize,
}

/// Represents a compressed HEX4 SXRC payload.
///
/// The HEX4 format supports larger, more complex offload tasks and is processed
/// via the Supernova hardware interface.
#[derive(Debug, Clone)]
pub struct Hex4Payload {
    /// The actual compressed data payload.
    pub data: [u8; 64],
    /// The size of the valid data in the payload.
    pub size: usize,
}

/// A unified SXRC payload type that can hold either a HEX2 or a HEX4 payload.
#[derive(Debug, Clone)]
pub enum SxrcPayload {
    /// A lightweight HEX2 payload.
    Hex2(Hex2Payload),
    /// A standard HEX4 payload.
    Hex4(Hex4Payload),
}
