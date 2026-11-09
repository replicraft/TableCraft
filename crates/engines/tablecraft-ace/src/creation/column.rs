//! Explicit native creation declarations, without row-driven type inference.
use super::CreateDatabaseError;
use tablecraft_core::values::Value;
use tablecraft_schema::{DecimalConstraints, ScalarType};

const SHORT_LIMIT: usize = 255;
const DESCRIPTION_UNITS: usize = 255;

/// One borrowed native declaration for fresh database creation.
///
/// Names and complete row/declaration compatibility are validated per operation.
/// Only explicit supported field policies/defaults and Increment declarations are optional; native flags,
/// offsets and mutation authority are not supplied.
///
/// Retain each returned declaration when choosing policies:
///
/// ```
/// use tablecraft_ace::CreateColumn;
/// use tablecraft_schema::ScalarType;
///
/// let column = CreateColumn::new("Score", ScalarType::Int32).with_required(true);
/// assert_eq!(column.required(), Some(true));
/// ```
///
/// Discarding a configured copy leaves the original declaration unchanged:
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use tablecraft_ace::CreateColumn;
/// use tablecraft_schema::ScalarType;
///
/// let column = CreateColumn::new("Score", ScalarType::Int32);
/// column.with_required(true);
/// ```
#[must_use = "retain the returned column declaration so its policies are applied"]
#[derive(Clone, Copy)]
pub struct CreateColumn<'a> {
    name: &'a str,
    description: Option<&'a str>,
    declaration: Declaration,
    required: Option<bool>,
    allow_zero_length: Option<bool>,
    default_literal: Option<&'a Value>,
    primary_key_name: Option<&'a str>,
    incrementing: bool,
    unicode_compression: bool,
}

#[derive(Clone, Copy)]
pub(super) enum Declaration {
    Fixed(ScalarType),
    ShortText { units: usize },
    ShortBinary { bytes: usize },
    Decimal(DecimalConstraints),
}

impl Declaration {
    pub const fn scalar(self) -> ScalarType {
        match self {
            Self::Fixed(scalar) => scalar,
            Self::ShortText { .. } => ScalarType::Text,
            Self::ShortBinary { .. } => ScalarType::Binary,
            Self::Decimal(_) => ScalarType::Decimal,
        }
    }

    pub fn validate(self) -> Result<(), CreateDatabaseError> {
        match self {
            Self::ShortText { units } if !(1..=SHORT_LIMIT).contains(&units) => {
                Err(CreateDatabaseError::Declaration)
            }
            Self::ShortBinary { bytes } if !(1..=SHORT_LIMIT).contains(&bytes) => {
                Err(CreateDatabaseError::Declaration)
            }
            _ => Ok(()),
        }
    }
}

impl<'a> CreateColumn<'a> {
    /// Describe an ordinary fixed carrier; variable families require explicit
    /// native parameters through the separate declaration factories below.
    pub const fn new(name: &'a str, scalar: ScalarType) -> Self {
        Self {
            name,
            description: None,
            declaration: Declaration::Fixed(scalar),
            required: None,
            allow_zero_length: None,
            default_literal: None,
            primary_key_name: None,
            incrementing: false,
            unicode_compression: false,
        }
    }

    /// Declare a positive native Text capacity of 1..=255 UTF-16 code units.
    /// This is the current constructive profile, not a statement that zero is
    /// impossible in every native format. Names are checked by the operation.
    ///
    /// # Errors
    ///
    /// Returns [`CreateDatabaseError::Declaration`] if the capacity is outside
    /// 1..=255 UTF-16 code units. Identifier and operation-level compatibility
    /// checks remain with database preparation.
    pub fn short_text(name: &'a str, max_utf16_units: usize) -> Result<Self, CreateDatabaseError> {
        let declaration = Declaration::ShortText {
            units: max_utf16_units,
        };
        declaration.validate()?;
        Ok(Self {
            name,
            description: None,
            declaration,
            required: None,
            allow_zero_length: None,
            default_literal: None,
            primary_key_name: None,
            incrementing: false,
            unicode_compression: false,
        })
    }

    /// Declare a positive Short Binary limit of 1..=255 bytes, independently of
    /// value contents or the wider reader-only legacy/system declaration ceiling.
    ///
    /// # Errors
    ///
    /// Returns [`CreateDatabaseError::Declaration`] if the byte capacity is
    /// outside 1..=255. Identifier and operation-level compatibility checks
    /// remain with database preparation.
    pub fn short_binary(name: &'a str, max_bytes: usize) -> Result<Self, CreateDatabaseError> {
        let declaration = Declaration::ShortBinary { bytes: max_bytes };
        declaration.validate()?;
        Ok(Self {
            name,
            description: None,
            declaration,
            required: None,
            allow_zero_length: None,
            default_literal: None,
            primary_key_name: None,
            incrementing: false,
            unicode_compression: false,
        })
    }

    /// Reuse validated native Decimal precision and exact retained scale.
    pub const fn decimal(name: &'a str, constraints: DecimalConstraints) -> Self {
        Self {
            name,
            description: None,
            declaration: Declaration::Decimal(constraints),
            required: None,
            allow_zero_length: None,
            default_literal: None,
            primary_key_name: None,
            incrementing: false,
            unicode_compression: false,
        }
    }

    /// Retain inert descriptive text, including present-empty text. The current
    /// constructive profile counts at most 255 UTF-16 units, without normalization.
    ///
    /// # Errors
    ///
    /// Returns [`CreateDatabaseError::Declaration`] if the description exceeds
    /// 255 UTF-16 code units. A present-empty description remains valid.
    pub fn with_description(mut self, description: &'a str) -> Result<Self, CreateDatabaseError> {
        if description
            .encode_utf16()
            .take(DESCRIPTION_UNITS + 1)
            .count()
            > DESCRIPTION_UNITS
        {
            return Err(CreateDatabaseError::Declaration);
        }
        self.description = Some(description);
        Ok(self)
    }

    /// Exact optional descriptive metadata; an empty description is still present.
    pub const fn description(self) -> Option<&'a str> {
        self.description
    }

    /// Author the exact Boolean Required policy; omission stays distinct from false.
    pub const fn with_required(mut self, required: bool) -> Self {
        self.required = Some(required);
        self
    }

    /// Author AllowZeroLength only on an explicitly sized native Short Text.
    /// This does not alter Binary, infer defaults or add Memo creation support.
    ///
    /// # Errors
    ///
    /// Returns [`CreateDatabaseError::Declaration`] unless this is an explicitly
    /// sized Short Text declaration, even when `allow` is false.
    pub fn with_allow_zero_length(mut self, allow: bool) -> Result<Self, CreateDatabaseError> {
        if !matches!(self.declaration, Declaration::ShortText { .. }) {
            return Err(CreateDatabaseError::Declaration);
        }
        self.allow_zero_length = Some(allow);
        Ok(self)
    }

    /// Explicitly allow native Unicode compression on a sized Short Text field.
    /// Existing constructors retain compression disabled. This metadata option
    /// neither changes text values nor authorizes imported-field mutations.
    ///
    /// # Errors
    ///
    /// Returns [`CreateDatabaseError::Declaration`] unless this is an explicitly
    /// sized Short Text declaration, even when `enabled` is false.
    pub fn with_unicode_compression(mut self, enabled: bool) -> Result<Self, CreateDatabaseError> {
        if !matches!(self.declaration, Declaration::ShortText { .. }) {
            return Err(CreateDatabaseError::Declaration);
        }
        self.unicode_compression = enabled;
        Ok(self)
    }

    /// Explicit native Short Text compression permission; false by default.
    pub const fn unicode_compression(self) -> bool {
        self.unicode_compression
    }

    /// Declare an exact typed scalar constant default. Nullable supported families
    /// accept NULL; ordinary GUID currently accepts only an explicit NULL default.
    ///
    /// This metadata never replaces a supplied value. Date declaration formatting
    /// must preserve every original bit or preparation refuses before publication.
    /// Clock, generated and executable expressions remain outside this API.
    ///
    /// # Errors
    ///
    /// Returns [`CreateDatabaseError::Declaration`] for an Increment field or a
    /// declaration without supported constant defaults. Returns
    /// [`CreateDatabaseError::Carrier`] for a mismatched scalar carrier,
    /// non-finite floating value or Decimal outside the declared precision/scale.
    /// Complete default formatting and physical compatibility are checked during
    /// preparation; this setter does not authorize a write.
    pub fn with_default_literal(mut self, value: &'a Value) -> Result<Self, CreateDatabaseError> {
        if self.incrementing {
            return Err(CreateDatabaseError::Declaration);
        }
        let compatible = match self.declaration {
            Declaration::Fixed(ScalarType::Boolean) => matches!(value, Value::Boolean(_)),
            Declaration::Fixed(ScalarType::Byte) => matches!(value, Value::Byte(_) | Value::Null),
            Declaration::Fixed(ScalarType::Int16) => matches!(value, Value::Int16(_) | Value::Null),
            Declaration::Fixed(ScalarType::Single) => match value {
                Value::Null => true,
                Value::Single(value) => f32::from_bits(value.to_bits()).is_finite(),
                _ => false,
            },
            Declaration::Fixed(ScalarType::Double) => match value {
                Value::Null => true,
                Value::Double(value) => f64::from_bits(value.to_bits()).is_finite(),
                _ => false,
            },
            Declaration::Fixed(ScalarType::Guid) => matches!(value, Value::Null),
            Declaration::Decimal(constraints) => match value {
                Value::Null => true,
                Value::Decimal(value) => constraints.validate(*value).is_ok(),
                _ => false,
            },
            Declaration::Fixed(ScalarType::Int32) => matches!(value, Value::Int32(_) | Value::Null),
            Declaration::Fixed(ScalarType::Int64) => matches!(value, Value::Int64(_) | Value::Null),
            Declaration::Fixed(ScalarType::Currency) => {
                matches!(value, Value::Currency(_) | Value::Null)
            }
            Declaration::Fixed(ScalarType::DateTime) => {
                matches!(value, Value::DateTime(_) | Value::Null)
            }
            Declaration::ShortText { .. } => matches!(value, Value::Text(_) | Value::Null),
            _ => return Err(CreateDatabaseError::Declaration),
        };
        if !compatible {
            return Err(CreateDatabaseError::Carrier);
        }
        self.default_literal = Some(value);
        Ok(self)
    }

    /// Original typed default, with absent declaration distinct from explicit NULL.
    pub const fn default_literal(self) -> Option<&'a Value> {
        self.default_literal
    }

    /// Declare one ascending Int32, Int64 or Currency primary key.
    ///
    /// The operation validates the identifier, uniqueness, non-NULL initial keys
    /// and physical fit. No page, map, native flags or generated values are supplied.
    ///
    /// # Errors
    ///
    /// Returns [`CreateDatabaseError::Declaration`] unless this is an ordinary
    /// Int32, Int64 or Currency declaration. The operation checks the key name
    /// and complete native key constraints separately.
    pub fn with_primary_key(mut self, name: &'a str) -> Result<Self, CreateDatabaseError> {
        if !matches!(
            self.declaration,
            Declaration::Fixed(ScalarType::Int32 | ScalarType::Int64 | ScalarType::Currency)
        ) {
            return Err(CreateDatabaseError::Declaration);
        }
        self.primary_key_name = Some(name);
        Ok(self)
    }

    /// Explicitly declare a fresh LONG Increment field, without issuing IDs.
    ///
    /// The operation requires an empty table, this field first with an explicit
    /// primary key, and no optional policies/defaults. Ordinary companions use
    /// the same bounded fixed and sized-variable declarations as manual creation.
    /// Complete initial rows are never rewritten or interpreted as generation.
    ///
    /// # Errors
    ///
    /// Returns [`CreateDatabaseError::Declaration`] unless this is Int32 with
    /// DefaultValue, Required and AllowZeroLength all absent. The operation
    /// separately validates the primary key, field order and empty source.
    pub fn with_incrementing_autonumber(mut self) -> Result<Self, CreateDatabaseError> {
        if !matches!(self.declaration, Declaration::Fixed(ScalarType::Int32))
            || self.default_literal.is_some()
            || self.required.is_some()
            || self.allow_zero_length.is_some()
        {
            return Err(CreateDatabaseError::Declaration);
        }
        self.incrementing = true;
        Ok(self)
    }

    /// Explicit creation metadata, not an imported-field mutation permission.
    pub const fn incrementing_autonumber(self) -> bool {
        self.incrementing
    }

    /// Original explicit primary identifier; absence creates no index.
    pub const fn primary_key_name(self) -> Option<&'a str> {
        self.primary_key_name
    }

    /// Actual explicitly supplied Required policy, without inferred false.
    pub const fn required(self) -> Option<bool> {
        self.required
    }

    /// Actual explicitly supplied empty Text policy, without inferred true.
    pub const fn allow_zero_length(self) -> Option<bool> {
        self.allow_zero_length
    }

    /// Original spelling without trimming, normalization or encoding conversion.
    pub const fn name(self) -> &'a str {
        self.name
    }

    /// Declared exact carrier; NULL never infers a type or native capacity.
    pub const fn scalar_type(self) -> ScalarType {
        self.declaration.scalar()
    }

    /// Native Text capacity in UTF-16 units, not Unicode scalar characters.
    pub const fn max_text_utf16_units(self) -> Option<usize> {
        match self.declaration {
            Declaration::ShortText { units } => Some(units),
            _ => None,
        }
    }

    /// Native Short Binary byte capacity, when explicitly declared.
    pub const fn max_binary_bytes(self) -> Option<usize> {
        match self.declaration {
            Declaration::ShortBinary { bytes } => Some(bytes),
            _ => None,
        }
    }

    /// Native Decimal precision/scale, without implicit rescaling policy.
    pub const fn decimal_constraints(self) -> Option<DecimalConstraints> {
        match self.declaration {
            Declaration::Decimal(constraints) => Some(constraints),
            _ => None,
        }
    }

    pub(super) const fn declaration(self) -> Declaration {
        self.declaration
    }
}
