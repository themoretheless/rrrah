//! The GPU API used across rrrah.
//!
//! On macOS this is Forma's `gpu-runtime` (Metal) with its `gpu-shader` WGSL compiler; on other
//! platforms it is upstream `wgpu` with `naga`. Depend on this crate as `wgpu`; the WGSL compiler
//! the target actually runs is available as [`naga`].

#[cfg(target_os = "macos")]
pub use gpu_runtime::*;
#[cfg(target_os = "macos")]
pub use gpu_shader as naga;

#[cfg(not(target_os = "macos"))]
pub use naga;
#[cfg(not(target_os = "macos"))]
pub use wgpu::*;
