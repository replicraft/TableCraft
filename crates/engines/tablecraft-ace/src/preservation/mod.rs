mod dependencies;
mod image;

pub use dependencies::{DependencyCoverage, ObjectDependency};
pub use image::NativeImage;

pub(crate) const COPY_CHUNK_BYTES: usize = 64 * 1024;
