/// Qualified native attachment payload wrapper encoding, not the file's format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentEncoding {
    /// Body bytes are stored directly.
    Uncompressed,
    /// One complete checked zlib stream contains the body.
    Zlib,
}

/// Decoded original attachment bytes. Never opens content or uses stored labels
/// as paths. Hosts own explicit extraction destinations and publication policy.
pub struct DecodedAttachment {
    pub(super) body: Vec<u8>,
    pub(super) data_start: usize,
    pub(super) type_label: String,
    pub(super) encoding: AttachmentEncoding,
}
impl DecodedAttachment {
    /// Original file bytes, without the native attachment wrapper.
    pub fn data(&self) -> &[u8] {
        &self.body[self.data_start..]
    }
    /// Wrapper type label with its single terminator removed; not a MIME or path
    /// validation result. Embedded Unicode/NUL content is retained.
    pub fn type_label(&self) -> &str {
        &self.type_label
    }
    /// Native storage compression, independent of the file content's encoding.
    pub const fn encoding(&self) -> AttachmentEncoding {
        self.encoding
    }
}
