//! Closed page purposes advance the same bounded cursor for real consumers.
use super::Edit;
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, format::page::PAGE_BYTES, header::check,
    limits::MAX_IMAGE_BYTES,
};
use tablecraft_core::limits::ResourceBudget;

enum Purpose {
    Data,
    OrdinaryCandidateBitmap { definition: u32, reference: u32 },
    Index { physical: u32, usage: u32 },
    IndexBitmap { physical: u32, usage: u32 },
}

struct Reservation {
    number: u32,
    purpose: Purpose,
}

pub(in crate::table) struct IndexReservation {
    pub number: u32,
    pub physical: u32,
    pub usage: u32,
}

pub(in crate::table) struct State {
    pub original_length: usize,
    pub length: usize,
    pub owner: u32,
    pub map_edits: Vec<Edit>,
    reservations: Vec<Reservation>,
}

impl State {
    pub(in crate::table) fn new(image: &NativeImage, owner: u32) -> Self {
        Self {
            original_length: image.as_bytes().len(),
            length: image.as_bytes().len(),
            owner,
            map_edits: Vec::new(),
            reservations: Vec::new(),
        }
    }

    pub(in crate::table) fn next(&self) -> Result<u32, EditError> {
        u32::try_from(self.length / PAGE_BYTES).map_err(|_| InspectError::StructureLimit.into())
    }

    pub(in crate::table) fn preview(&self, pages: usize) -> Result<usize, EditError> {
        pages
            .checked_mul(PAGE_BYTES)
            .and_then(|added| self.length.checked_add(added))
            .filter(|length| *length as u64 <= MAX_IMAGE_BYTES)
            .ok_or_else(|| InspectError::StructureLimit.into())
    }

    pub(in crate::table) fn reserve_data(
        &mut self,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.commit(number, &[Purpose::Data], budget, cancelled)
    }

    pub(in crate::table) fn reserve_index(
        &mut self,
        number: u32,
        index: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.commit(
            number,
            &[Purpose::Index {
                physical: index.data_number(),
                usage: index.usage_map(),
            }],
            budget,
            cancelled,
        )
    }

    pub(in crate::table) fn reserve_candidate_bitmap(
        &mut self,
        number: u32,
        reference: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.commit(
            number,
            &[Purpose::OrdinaryCandidateBitmap {
                definition: self.owner,
                reference,
            }],
            budget,
            cancelled,
        )
    }

    pub(in crate::table) fn reserve_index_bitmap(
        &mut self,
        number: u32,
        index: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.commit(
            number,
            &[
                Purpose::Index {
                    physical: index.data_number(),
                    usage: index.usage_map(),
                },
                Purpose::IndexBitmap {
                    physical: index.data_number(),
                    usage: index.usage_map(),
                },
            ],
            budget,
            cancelled,
        )
    }

    fn commit(
        &mut self,
        number: u32,
        purposes: &[Purpose],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.next()? != number
            || self.length < self.original_length
            || !self.length.is_multiple_of(PAGE_BYTES)
        {
            return Err(EditError::Preservation);
        }

        let length = self.preview(purposes.len())?;
        budget
            .charge_allocation(purposes.len() as u64, size_of::<Reservation>() as u64)
            .map_err(InspectError::from)?;
        self.reservations
            .try_reserve(purposes.len())
            .map_err(|_| InspectError::Allocation)?;
        let count = u32::try_from(purposes.len()).map_err(|_| InspectError::StructureLimit)?;
        number
            .checked_add(count)
            .ok_or(InspectError::StructureLimit)?;

        // Prepay cancellation before committing either member of a pair.
        for _ in purposes {
            check(budget, cancelled)?;
        }

        for (position, purpose) in purposes.iter().enumerate() {
            let purpose = match purpose {
                Purpose::Data => Purpose::Data,
                Purpose::OrdinaryCandidateBitmap {
                    definition,
                    reference,
                } => Purpose::OrdinaryCandidateBitmap {
                    definition: *definition,
                    reference: *reference,
                },
                Purpose::Index { physical, usage } => Purpose::Index {
                    physical: *physical,
                    usage: *usage,
                },
                Purpose::IndexBitmap { physical, usage } => Purpose::IndexBitmap {
                    physical: *physical,
                    usage: *usage,
                },
            };
            self.reservations.push(Reservation {
                number: number
                    .checked_add(position as u32)
                    .ok_or(InspectError::StructureLimit)?,
                purpose,
            });
        }
        self.length = length;

        Ok(())
    }

    pub(in crate::table) fn reservations_len(&self) -> usize {
        self.reservations.len()
    }

    fn reservation(
        &self,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<&Reservation, EditError> {
        check(budget, cancelled)?;
        let original = u32::try_from(self.original_length / PAGE_BYTES)
            .map_err(|_| EditError::Preservation)?;
        let position = number
            .checked_sub(original)
            .ok_or(EditError::Preservation)?;
        let item = self
            .reservations
            .get(position as usize)
            .ok_or(EditError::Preservation)?;
        if item.number != number {
            return Err(EditError::Preservation);
        }

        Ok(item)
    }

    pub(in crate::table) fn require_data(
        &self,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if !matches!(
            self.reservation(number, budget, cancelled)?.purpose,
            Purpose::Data
        ) {
            return Err(EditError::Preservation);
        }

        Ok(())
    }

    pub(in crate::table) fn require_index_bitmap(
        &self,
        number: u32,
        physical: u32,
        usage: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        match self.reservation(number, budget, cancelled)?.purpose {
            Purpose::IndexBitmap {
                physical: expected,
                usage: reference,
            } if expected == physical && reference == usage => Ok(()),
            _ => Err(EditError::Preservation),
        }
    }

    pub(in crate::table) fn require_candidate_bitmap(
        &self,
        number: u32,
        definition: u32,
        reference: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        match self.reservation(number, budget, cancelled)?.purpose {
            Purpose::OrdinaryCandidateBitmap {
                definition: actual_definition,
                reference: actual_reference,
            } if actual_definition == definition && actual_reference == reference => Ok(()),
            _ => Err(EditError::Preservation),
        }
    }

    pub(in crate::table) fn indexes(&self) -> impl Iterator<Item = IndexReservation> + '_ {
        self.reservations
            .iter()
            .filter_map(|reservation| match reservation.purpose {
                Purpose::Index { physical, usage } => Some(IndexReservation {
                    number: reservation.number,
                    physical,
                    usage,
                }),
                _ => None,
            })
    }
}
