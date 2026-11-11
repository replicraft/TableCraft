//! Original nonunique LONG index intent and shared byte/declaration mechanics.
pub(in crate::table) mod buffer;
pub(in crate::table::mutation) mod definition;

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
