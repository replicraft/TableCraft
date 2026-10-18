mod decode;
mod inflate;
mod metadata;
pub use decode::decode_attachment;
pub use metadata::{AttachmentEncoding, DecodedAttachment};
