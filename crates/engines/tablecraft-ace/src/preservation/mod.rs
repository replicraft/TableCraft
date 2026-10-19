mod children;
mod dependencies;
mod image;
mod payload_data;
mod payloads;

pub use dependencies::{DependencyCoverage, ObjectDependency};
pub use image::NativeImage;
pub use payload_data::{ObjectPayloadData, OpaquePayload};
pub use payloads::{ObjectPayloads, PayloadPresence};

pub(crate) const COPY_CHUNK_BYTES: usize = 64 * 1024;
