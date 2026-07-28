//! MaRTix structures for GMS routing.
//!
//! This module defines the compute backend, routing commands, and the
//! primary payload structure used by the Supernova driver to route
//! compute tasks to various hardware backends.

use crate::sxrc_core::SxrcPayload;

/// Represents the compute backend to be used for a MaRTix task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeBackend {
    /// CUDA backend, utilizing cudarc.
    Cuda,
    /// Raw Vulkan backend.
    Vulkan,
    /// Raw Metal backend (primarily for Mac environments).
    Metal,
}

/// Commands available for the MaRTix RT core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RTCoreCommand {
    /// Perform ray tracing operations.
    RayTrace,
    /// Perform matrix multiplication operations.
    MatrixMultiply,
}

/// The payload sent to the MaRTix subsystem for processing.
///
/// Contains routing information (the compute backend and command)
/// as well as the memory-optimized SXRC payload.
#[derive(Debug, Clone)]
pub struct MartixPayload {
    /// The target compute backend.
    pub backend: ComputeBackend,
    /// The command to execute on the backend.
    pub command: RTCoreCommand,
    /// The compressed SXRC memory payload.
    pub payload: SxrcPayload,
}
