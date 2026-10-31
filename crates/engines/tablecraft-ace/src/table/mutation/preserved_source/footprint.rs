//! Source-bound protection chosen before any record inspection or mutation.
use super::PayloadAdmission;
#[cfg(feature = "ace16-write-preview")]
use crate::format::page::PAGE_BYTES;
#[cfg(feature = "journal")]
use crate::table::mutation::normal_before_change::protection::Protection;
#[cfg(feature = "ace16-write-preview")]
use crate::table::mutation::variable_split::metadata::Metadata;
use crate::{EditError, NativeImage, NativeIndex, TableSchema};
use tablecraft_core::limits::ResourceBudget;

pub(super) enum Footprint {
    #[cfg(feature = "ace16-write-preview")]
    Engineering(Metadata),
    #[cfg(feature = "journal")]
    Normal(Protection),
}

impl Footprint {
    pub(super) fn capture(
        image: &NativeImage,
        schema: &TableSchema,
        _indexes: &[NativeIndex],
        payload: &PayloadAdmission<'_, '_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        match payload {
            #[cfg(feature = "journal")]
            PayloadAdmission::NativeSelected(_) => Ok(Self::Normal(Protection::capture(
                image, schema, budget, cancelled,
            )?)),
            #[cfg(feature = "ace16-write-preview")]
            PayloadAdmission::Inert | PayloadAdmission::Selected(_) => {
                let metadata = Metadata::capture_for_preserved_boolean(
                    image, schema, _indexes, budget, cancelled,
                )?;
                metadata.require_source(image, schema.definition_page())?;
                Ok(Self::Engineering(metadata))
            }
        }
    }

    pub(super) fn require_record(
        &self,
        _image: &NativeImage,
        _schema: &TableSchema,
        offset: usize,
        _length: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        match self {
            #[cfg(feature = "ace16-write-preview")]
            Self::Engineering(metadata) => {
                metadata.require_data_page((offset / PAGE_BYTES) as u32, budget, cancelled)
            }
            #[cfg(feature = "journal")]
            Self::Normal(protection) => {
                protection.require(_image, _schema, offset, _length, budget, cancelled)
            }
        }
    }
}
