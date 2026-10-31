//! Borrowed creation declarations; exact native qualification occurs per operation.
use tablecraft_schema::ScalarType;

/// One named ordinary fixed-scalar declaration for fresh database creation.
///
/// The creation operation validates names and admits only qualified native
/// families. A NULL value never infers this declaration. This request type
/// neither supplies native flags/offsets nor declares Required or validation rules.
#[derive(Clone, Copy)]
pub struct CreateColumn<'a> {
    name: &'a str,
    scalar: ScalarType,
}

impl<'a> CreateColumn<'a> {
    /// Describe a field without allocating or converting names or scalar types.
    /// Unsupported native families are rejected by the creation operation,
    /// including for empty tables and all-NULL inputs.
    pub const fn new(name: &'a str, scalar: ScalarType) -> Self {
        Self { name, scalar }
    }

    /// Original field spelling, without trimming or normalization.
    pub const fn name(self) -> &'a str {
        self.name
    }

    /// Declared carrier for present values; Boolean does not admit a NULL carrier.
    pub const fn scalar_type(self) -> ScalarType {
        self.scalar
    }
}
