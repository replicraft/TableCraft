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

/// Native source policy retained privately in every successful creation effect.
#[cfg(all(feature = "journal", feature = "ace16-create"))]
#[derive(Clone, Copy)]
pub(super) enum IndexPolicy {
    Ordinary,
    UniqueLong,
}

/// A unique secondary manual LONG request. PRIMARY, Required and IgnoreNulls
/// are not options in this language; consuming qualification remains mandatory.
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub struct UniqueLongIndexCreate<'a> {
    ordinary: SecondaryIndexCreate<'a>,
}

#[cfg(all(feature = "journal", feature = "ace16-create"))]
impl<'a> UniqueLongIndexCreate<'a> {
    /// Borrow exact original names and direction without granting write authority.
    pub const fn new(name: &'a str, column: &'a str, descending: bool) -> Self {
        Self {
            ordinary: SecondaryIndexCreate::new(name, column, descending),
        }
    }

    pub(super) fn ordinary(&self) -> &SecondaryIndexCreate<'a> {
        &self.ordinary
    }
}
