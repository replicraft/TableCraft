//! A captured schema family stays explicit through preparation and AFTER proof.
use super::{Error, Family, Source, Table, observe, retained};
use crate::{InspectError, NativeImage};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

impl Family {
    pub(crate) fn capture(
        self,
        image: NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Source, Error> {
        let table = self.observe(&image, table, budget, cancelled)?;
        Ok(Source { image, table })
    }

    pub(crate) fn observe(
        self,
        image: &NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Table, Error> {
        match self {
            Self::Current => observe(image, table, budget, cancelled),
            Self::Retained => retained::observe(image, table, budget, cancelled),
        }
    }

    pub(crate) fn image(
        self,
        bytes: &[u8],
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeImage, InspectError> {
        let mut input = Cursor::new(bytes);
        match self {
            Self::Current => NativeImage::capture(&mut input, scope, budget, cancelled),
            Self::Retained => NativeImage::capture_retained(&mut input, scope, budget, cancelled),
        }
    }
}
