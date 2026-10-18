mod dependencies;
mod image;
mod payloads;

pub use dependencies::{DependencyCoverage, ObjectDependency};
pub use image::NativeImage;
pub use payloads::{ObjectPayloads, PayloadPresence};

pub(crate) const COPY_CHUNK_BYTES: usize = 64 * 1024;
