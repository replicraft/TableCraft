use super::FieldRenameError as Error;
use crate::table::mutation::saved_field::Observed;
use crate::{NativeImage, creation::FieldHistoryOffSource};
use tablecraft_core::limits::ResourceBudget;

/// Its private source can only come from the complete shared settled proof.
pub(super) struct Qualified {
    proof: FieldHistoryOffSource,
    pub observed: Observed,
}

impl Qualified {
    pub fn capture(
        image: NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let proof = FieldHistoryOffSource::capture(image, budget, cancelled)?;
        let observed = Observed::capture(proof.image(), table, field, budget, cancelled)
            .map_err(Error::from)?;
        Ok(Self { proof, observed })
    }

    pub fn image(&self) -> &NativeImage {
        self.proof.image()
    }
}
