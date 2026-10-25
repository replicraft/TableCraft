//! Static successful-result carriers for the two independent execution profiles.
use crate::OutputType;

#[derive(Clone, Copy)]
pub(crate) enum ResultProfile {
    Materialized,
    Declared,
}

#[derive(Clone, Copy)]
pub(crate) struct ProfiledType {
    pub materialized: OutputType,
    pub declared: OutputType,
}

impl ProfiledType {
    pub const fn uniform(value: OutputType) -> Self {
        Self {
            materialized: value,
            declared: value,
        }
    }

    pub const fn selected(self, profile: ResultProfile) -> OutputType {
        match profile {
            ResultProfile::Materialized => self.materialized,
            ResultProfile::Declared => self.declared,
        }
    }

    pub fn conservative(self) -> OutputType {
        if self.materialized == self.declared {
            self.materialized
        } else if let (
            OutputType::Scalar {
                data_type: left,
                nullable: a,
            },
            OutputType::Scalar {
                data_type: right,
                nullable: b,
            },
        ) = (self.materialized, self.declared)
            && left == right
        {
            OutputType::Scalar {
                data_type: left,
                nullable: a || b,
            }
        } else {
            OutputType::Dynamic
        }
    }

    pub fn with_null(self) -> Self {
        Self {
            materialized: self.materialized.with_null(),
            declared: self.declared.with_null(),
        }
    }
}
