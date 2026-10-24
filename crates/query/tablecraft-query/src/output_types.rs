//! Conservative output metadata, independent of stored column identity.
mod union;
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};

/// Static information about an output in the implemented evaluation profile.
/// This is not native Access SQL type inference or a storage schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputType {
    /// Every successful value is SQL NULL. This is expression metadata, not a
    /// stored field type, an Empty state or an implicit conversion target.
    Null,
    /// A checked scalar carrier and supported constraints.
    Scalar {
        /// Carrier and value constraints retained without inspecting records.
        data_type: TypeDescriptor,
        /// NULL may occur, including through an outer join or an empty aggregate.
        nullable: bool,
    },
    /// No single checked scalar descriptor is established. This can include
    /// varying carriers, expression-only states or unimplemented inference.
    Dynamic,
}

impl OutputType {
    pub(crate) fn column(column: &Column) -> Self {
        Self::Scalar {
            data_type: column.type_descriptor(),
            nullable: column.nullable(),
        }
    }
    pub(crate) fn scalar(data_type: ScalarType, nullable: bool) -> Self {
        Self::Scalar {
            data_type: TypeDescriptor::new(data_type),
            nullable,
        }
    }
    pub(crate) fn with_null(self) -> Self {
        match self {
            Self::Scalar { data_type, .. } => Self::Scalar {
                data_type,
                nullable: true,
            },
            Self::Null => Self::Null,
            Self::Dynamic => Self::Dynamic,
        }
    }
    pub(crate) fn union_result(self, other: Self) -> Self {
        union::common(self, other)
    }
    // Selectors retain their chosen value: only same-carrier descriptors may
    // share the common constraint rules without introducing a value conversion.
    pub(crate) fn union(self, other: Self) -> Self {
        match (self, other) {
            (Self::Null, other) | (other, Self::Null) => other.with_null(),
            (
                Self::Scalar {
                    data_type: left,
                    nullable: a,
                },
                Self::Scalar {
                    data_type: right,
                    nullable: b,
                },
            ) if left.scalar_type() == right.scalar_type() => union::common(
                Self::Scalar {
                    data_type: left,
                    nullable: a,
                },
                Self::Scalar {
                    data_type: right,
                    nullable: b,
                },
            ),
            _ => Self::Dynamic,
        }
    }
}
