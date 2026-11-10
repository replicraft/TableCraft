use super::FieldRenameError as Error;
use crate::table::mutation::saved_field::Observed;
use crate::{NativeImage, creation::FieldHistoryOffSource};
use tablecraft_core::limits::ResourceBudget;

enum Proof {
    Empty(FieldHistoryOffSource),
    Populated(NativeImage),
    Current(NativeImage),
}

/// Its private source must pass complete selected-family reconstruction.
pub(super) struct Qualified {
    proof: Proof,
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
        Ok(Self {
            proof: Proof::Empty(proof),
            observed,
        })
    }

    pub(super) fn from_populated(image: NativeImage, observed: Observed) -> Self {
        Self {
            proof: Proof::Populated(image),
            observed,
        }
    }

    pub(super) fn from_current(image: NativeImage, observed: Observed) -> Self {
        Self {
            proof: Proof::Current(image),
            observed,
        }
    }

    pub(super) fn current_metadata(&self) -> bool {
        matches!(self.proof, Proof::Current(_))
    }

    pub fn image(&self) -> &NativeImage {
        match &self.proof {
            Proof::Empty(proof) => proof.image(),
            Proof::Populated(image) | Proof::Current(image) => image,
        }
    }
}
