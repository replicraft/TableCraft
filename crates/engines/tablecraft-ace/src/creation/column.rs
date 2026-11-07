//! Explicit native creation declarations, without row-driven type inference.
use super::CreateDatabaseError;
use tablecraft_core::values::Value;
use tablecraft_schema::{DecimalConstraints, ScalarType};

const SHORT_LIMIT: usize = 255;

/// One borrowed native declaration for fresh database creation.
///
/// Names and complete row/declaration compatibility are validated per operation.
/// Only explicit supported field policies/defaults and Increment declarations are optional; native flags,
/// offsets and mutation authority are not supplied.
#[derive(Clone, Copy)]
pub struct CreateColumn<'a> {
    name: &'a str,
    declaration: Declaration,
    required: Option<bool>,
    allow_zero_length: Option<bool>,
    default_literal: Option<&'a Value>,
    primary_key_name: Option<&'a str>,
    incrementing: bool,
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
            declaration: Declaration::Fixed(scalar),
            required: None,
            allow_zero_length: None,
            default_literal: None,
            primary_key_name: None,
            incrementing: false,
        }
    }

    /// Declare a positive native Text capacity of 1..=255 UTF-16 code units.
    /// This is the current constructive profile, not a statement that zero is
    /// impossible in every native format. Names are checked by the operation.
    pub fn short_text(name: &'a str, max_utf16_units: usize) -> Result<Self, CreateDatabaseError> {
        let declaration = Declaration::ShortText {
            units: max_utf16_units,
        };
        declaration.validate()?;
        Ok(Self {
            name,
            declaration,
            required: None,
            allow_zero_length: None,
            default_literal: None,
            primary_key_name: None,
            incrementing: false,
        })
    }

    /// Declare a positive Short Binary limit of 1..=255 bytes, independently of
    /// value contents or the wider reader-only legacy/system declaration ceiling.
    pub fn short_binary(name: &'a str, max_bytes: usize) -> Result<Self, CreateDatabaseError> {
        let declaration = Declaration::ShortBinary { bytes: max_bytes };
        declaration.validate()?;
        Ok(Self {
            name,
            declaration,
            required: None,
            allow_zero_length: None,
            default_literal: None,
            primary_key_name: None,
            incrementing: false,
        })
    }

    /// Reuse validated native Decimal precision and exact retained scale.
    pub const fn decimal(name: &'a str, constraints: DecimalConstraints) -> Self {
        Self {
            name,
            declaration: Declaration::Decimal(constraints),
            required: None,
            allow_zero_length: None,
            default_literal: None,
            primary_key_name: None,
            incrementing: false,
        }
    }

    /// Author the exact Boolean Required policy; omission stays distinct from false.
    pub const fn with_required(mut self, required: bool) -> Self {
        self.required = Some(required);
        self
    }

    /// Author AllowZeroLength only on an explicitly sized native Short Text.
    /// This does not alter Binary, infer defaults or add Memo creation support.
    pub fn with_allow_zero_length(mut self, allow: bool) -> Result<Self, CreateDatabaseError> {
        if !matches!(self.declaration, Declaration::ShortText { .. }) {
            return Err(CreateDatabaseError::Declaration);
        }
        self.allow_zero_length = Some(allow);
        Ok(self)
    }

    /// Declare an exact constant default for Boolean, Int32, Int64, Currency,
    /// reversible legacy Date or sized Short Text. Nullable families accept NULL.
    ///
    /// This metadata never replaces a supplied value. Date declaration formatting
    /// must preserve every original bit or preparation refuses before publication.
    /// Clock, generated and executable expressions remain outside this API.
    pub fn with_default_literal(mut self, value: &'a Value) -> Result<Self, CreateDatabaseError> {
        if self.incrementing {
            return Err(CreateDatabaseError::Declaration);
        }
        let compatible = match self.declaration {
            Declaration::Fixed(ScalarType::Boolean) => matches!(value, Value::Boolean(_)),
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
