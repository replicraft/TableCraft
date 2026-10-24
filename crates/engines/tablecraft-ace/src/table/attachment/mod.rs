mod decode;
mod inflate;
mod metadata;
pub use decode::{decode_attachment, decode_attachment_with_limits};
pub use metadata::{AttachmentEncoding, DecodedAttachment};

mod session;
pub use session::AttachmentDecoder;

mod limits;
pub use limits::AttachmentDecodeLimits;
