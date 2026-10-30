use crate::NativeImage;
use tablecraft_schema::SchemaName;

/// One ordinary nonunique Int32 secondary index. No PRIMARY, foreign, UNIQUE,
/// Required or IgnoreNULL policy is implied or available through this request.
pub struct SecondaryIndexCreate<'a> {
    pub(super) name: &'a str,
    pub(super) column: &'a str,
    pub(super) descending: bool,
}

impl<'a> SecondaryIndexCreate<'a> {
    /// Keep original caller intent borrowed; qualification occurs on the image.
    pub const fn new(name: &'a str, column: &'a str, descending: bool) -> Self {
        Self {
            name,
            column,
            descending,
        }
    }
}

/// Complete independently checked engineering bytes, without journal/save
/// conversion. Creating schema does not report changed data rows.
pub struct Ace16IndexCreatePreview {
    pub(super) before: NativeImage,
    pub(super) after: Vec<u8>,
    pub(super) name: SchemaName,
    pub(super) logical: u32,
    pub(super) physical: u32,
    pub(super) indexed_rows: usize,
}

impl Ace16IndexCreatePreview {
    /// Exact complete original image.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    /// Audited full candidate, never normal write permission.
    pub fn candidate_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Bound new logical index name.
    pub fn index_name(&self) -> &str {
        self.name.as_str()
    }

    /// New logical descriptor number, separate from physical index data.
    pub const fn logical_index_number(&self) -> u32 {
        self.logical
    }

    /// Newly created physical index-data number.
    pub const fn physical_index_number(&self) -> u32 {
        self.physical
    }

    /// All original logical rows indexed, including NULL and duplicate keys.
    pub const fn indexed_rows(&self) -> usize {
        self.indexed_rows
    }
}
