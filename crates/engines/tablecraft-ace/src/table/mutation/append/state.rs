//! Closed page purposes advance the same bounded cursor for real consumers.
use super::{
    Edit, global,
    owners::{OwnerContext, fresh_index::FreshIndexOwner},
};
use crate::{
    EditError, InspectError, LongValueMapReferences, NativeImage, NativeIndex,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, RepresentedMap},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    limits::MAX_IMAGE_BYTES,
    table::mutation::{
        map_authority::{self, Owner, RecordIdentity},
        variable_split::metadata::Metadata,
    },
};
use std::io::Cursor;
use std::ops::{Deref, DerefMut};
use tablecraft_core::limits::ResourceBudget;

enum Purpose {
    Data,
    GlobalBitmap {
        reference: u32,
        block: u32,
    },
    LongValueMapContainer {
        definition: u32,
        column: usize,
    },
    LongValue {
        column: usize,
        owned: u32,
        candidate: u32,
    },
    LongValueOwnedBitmap {
        column: usize,
        owned: u32,
        block: u32,
    },
    OrdinaryOwnedBitmap {
        definition: u32,
        reference: u32,
        block: u32,
    },
    OrdinaryCandidateBitmap {
        definition: u32,
        reference: u32,
    },
    CreatedIndexMapContainer {
        definition: u32,
        physical: u32,
    },
    CreatedIndexBody {
        definition: u32,
        physical: u32,
        usage: u32,
    },
    Index {
        definition: u32,
        physical: u32,
        usage: u32,
    },
    IndexBitmap {
        definition: u32,
        physical: u32,
        usage: u32,
    },
}

struct Reservation {
    number: u32,
    purpose: Purpose,
}

pub(in crate::table) struct IndexReservation {
    pub number: u32,
    pub definition: u32,
    pub physical: u32,
    pub usage: u32,
}

pub(in crate::table) struct CreatedIndexMapReservation {
    pub number: u32,
    pub definition: u32,
    pub physical: u32,
}

pub(in crate::table) struct LongValueReservation {
    pub number: u32,
    pub column: usize,
    pub owned: u32,
    pub candidate: u32,
}

pub(in crate::table) struct LongValueOwnedBitmapReservation {
    pub number: u32,
    pub column: usize,
    pub owned: u32,
    pub block: u32,
}

pub(in crate::table) struct LongValueMapContainerReservation {
    pub number: u32,
    pub definition: u32,
    pub column: usize,
}

pub(in crate::table) struct Ledger {
    pub original_length: usize,
    pub length: usize,
    pub map_edits: Vec<Edit>,
    reservations: Vec<Reservation>,
    global_configured: bool,
    global: Option<global::Growth>,
    global_reserved: bool,
}

pub(in crate::table) struct State<O = u32> {
    pub owner: O,
    ledger: Ledger,
    source_metadata: Option<Metadata>,
}

impl<O> Deref for State<O> {
    type Target = Ledger;

    fn deref(&self) -> &Ledger {
        &self.ledger
    }
}

impl<O> DerefMut for State<O> {
    fn deref_mut(&mut self) -> &mut Ledger {
        &mut self.ledger
    }
}

impl State {
    pub(in crate::table) fn new(image: &NativeImage, owner: u32) -> Self {
        Self::from_owner(image, owner)
    }
}

impl<O: OwnerContext> State<O> {
    pub(in crate::table) fn with_owner(image: &NativeImage, owner: O) -> Result<Self, EditError> {
        owner.census(image)?;
        Ok(Self::from_owner(image, owner))
    }

    fn from_owner(image: &NativeImage, owner: O) -> Self {
        Self {
            owner,
            ledger: Ledger {
                original_length: image.as_bytes().len(),
                length: image.as_bytes().len(),
                map_edits: Vec::new(),
                reservations: Vec::new(),
                global_configured: false,
                global: None,
                global_reserved: false,
            },
            source_metadata: None,
        }
    }

    pub(in crate::table) fn index_definition(
        &self,
        index: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<u32, EditError> {
        self.owner.index_definition(index, budget, cancelled)
    }

    pub(in crate::table) fn require_source_map_exclusive(
        &self,
        image: &NativeImage,
        owners: &[Owner],
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.tick()?;
        if let Some(metadata) = &self.source_metadata {
            metadata.require_source(image, self.owner.single()?)?;
            return metadata.require_exclusive(owners, reader.budget, reader.cancelled);
        }

        if let Some(census) = self.owner.census(image)? {
            return census.require_exclusive(owners, reader.budget, reader.cancelled);
        }

        let census = map_authority::capture_catalog(
            image,
            reader.budget,
            reader.cancelled,
            |_, _, _, _| Ok(()),
        )?;
        census.require_exclusive(owners, reader.budget, reader.cancelled)
    }

    pub(in crate::table) fn require_source_map_rewrite(
        &self,
        image: &NativeImage,
        owners: &[Owner],
        record: RecordIdentity,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.tick()?;
        if let Some(metadata) = &self.source_metadata {
            metadata.require_source(image, self.owner.single()?)?;
            metadata.require_exclusive(owners, reader.budget, reader.cancelled)?;
            return metadata.require_record_rewrite(
                record.owner,
                record.reference,
                record.range,
                reader.budget,
                reader.cancelled,
            );
        }

        if let Some(census) = self.owner.census(image)? {
            return census.require_record_rewrite(owners, record, reader.budget, reader.cancelled);
        }

        let census = map_authority::capture_catalog(
            image,
            reader.budget,
            reader.cancelled,
            |_, _, _, _| Ok(()),
        )?;
        census.require_record_rewrite(owners, record, reader.budget, reader.cancelled)
    }

    pub(in crate::table) fn enable_global_growth(
        &mut self,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
        certify: impl FnOnce(RecordIdentity, &mut Reader<'_, Cursor<&[u8]>>) -> Result<(), EditError>,
    ) -> Result<(), EditError> {
        if !self.global_configured {
            self.global = global::Growth::capture(reader, certify)?;
            self.global_configured = true;
        }
        Ok(())
    }

    pub(in crate::table) fn global_is_free(
        &self,
        original: &RepresentedMap,
        number: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<bool, EditError> {
        if let Some(global) = &self.global
            && (global.bitmap().is_some() || number >= global::INLINE_PAGES)
        {
            return global.is_free(number, reader);
        }
        let cell = original.future_cell(reader, number)?;
        super::current(&self.map_edits, &cell, reader)
    }

    pub(in crate::table) fn claim_global(
        &mut self,
        original: &RepresentedMap,
        number: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        let ledger = &mut self.ledger;
        if let Some(global) = &mut ledger.global
            && (global.bitmap().is_some() || number >= global::INLINE_PAGES)
        {
            return global.claim(number, &mut ledger.map_edits, reader);
        }
        let cell = original.future_cell(reader, number)?;
        if !super::current(&self.map_edits, &cell, reader)? {
            return Err(EditError::Preservation);
        }
        super::claim(&mut self.map_edits, cell, false, reader)
    }

    pub(in crate::table) fn global_record(&self) -> Option<&global::GlobalRecord> {
        self.global.as_ref().and_then(global::Growth::record)
    }

    pub(in crate::table) fn global_bitmap(&self) -> Option<&global::GlobalBitmap> {
        self.global.as_ref().and_then(global::Growth::bitmap)
    }

    #[cfg(test)]
    pub(in crate::table::mutation) fn global_record_mut(
        &mut self,
    ) -> Option<&mut global::GlobalRecord> {
        self.global.as_mut().and_then(global::Growth::record_mut)
    }

    #[cfg(test)]
    pub(in crate::table::mutation) fn global_bitmap_mut(
        &mut self,
    ) -> Option<&mut global::GlobalBitmap> {
        self.global.as_mut().and_then(global::Growth::bitmap_mut)
    }

    pub(in crate::table) fn planned_number(&self, position: usize) -> Result<u32, EditError> {
        let base = self.next()?;
        let position = u32::try_from(position).map_err(|_| InspectError::StructureLimit)?;
        let number = base
            .checked_add(position)
            .ok_or(InspectError::StructureLimit)?;
        if self.global.is_some() && !self.global_reserved && number > base.max(global::INLINE_PAGES)
        {
            number
                .checked_add(1)
                .ok_or(InspectError::StructureLimit.into())
        } else {
            Ok(number)
        }
    }

    pub(in crate::table) fn next(&self) -> Result<u32, EditError> {
        u32::try_from(self.length / PAGE_BYTES).map_err(|_| InspectError::StructureLimit.into())
    }

    pub(in crate::table) fn preview(&self, pages: usize) -> Result<usize, EditError> {
        let extra = usize::from(
            pages != 0
                && self.global.is_some()
                && !self.global_reserved
                && self
                    .next()?
                    .checked_add(
                        u32::try_from(pages - 1).map_err(|_| InspectError::StructureLimit)?,
                    )
                    .ok_or(InspectError::StructureLimit)?
                    >= global::INLINE_PAGES,
        );
        pages
            .checked_add(extra)
            .and_then(|pages| pages.checked_mul(PAGE_BYTES))
            .and_then(|added| self.length.checked_add(added))
            .filter(|length| *length as u64 <= MAX_IMAGE_BYTES)
            .ok_or_else(|| InspectError::StructureLimit.into())
    }

    pub(in crate::table) fn require_long_value_map_container(
        &self,
        number: u32,
        definition: u32,
        column: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        match self.reservation(number, budget, cancelled)?.purpose {
            Purpose::LongValueMapContainer {
                definition: actual,
                column: actual_column,
            } if actual == definition && actual_column == column => Ok(()),
            _ => Err(EditError::Preservation),
        }
    }

    pub(in crate::table) fn long_value_map_containers(
        &self,
    ) -> impl Iterator<Item = LongValueMapContainerReservation> + '_ {
        self.reservations
            .iter()
            .filter_map(|reservation| match reservation.purpose {
                Purpose::LongValueMapContainer { definition, column } => {
                    Some(LongValueMapContainerReservation {
                        number: reservation.number,
                        definition,
                        column,
                    })
                }
                _ => None,
            })
    }

    pub(in crate::table) fn reserve_index(
        &mut self,
        number: u32,
        index: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let definition = self.index_definition(index, budget, cancelled)?;
        self.commit(
            number,
            &[Purpose::Index {
                definition,
                physical: index.data_number(),
                usage: index.usage_map(),
            }],
            budget,
            cancelled,
        )
    }

    pub(in crate::table) fn require_owned_bitmap(
        &self,
        number: u32,
        definition: u32,
        reference: u32,
        block: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        match self.reservation(number, budget, cancelled)?.purpose {
            Purpose::OrdinaryOwnedBitmap {
                definition: actual,
                reference: actual_reference,
                block: actual_block,
            } if actual == definition && actual_reference == reference && actual_block == block => {
                Ok(())
            }
            _ => Err(EditError::Preservation),
        }
    }

    pub(in crate::table) fn owned_bitmaps(
        &self,
    ) -> impl Iterator<Item = (u32, u32, u32, u32)> + '_ {
        self.reservations
            .iter()
            .filter_map(|reservation| match reservation.purpose {
                Purpose::OrdinaryOwnedBitmap {
                    definition,
                    reference,
                    block,
                } => Some((reservation.number, definition, reference, block)),
                _ => None,
            })
    }

    pub(in crate::table) fn reserve_index_bitmap(
        &mut self,
        number: u32,
        index: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<u32, EditError> {
        let definition = self.index_definition(index, budget, cancelled)?;
        let metadata = self.planned_number(1)?;
        self.commit(
            number,
            &[
                Purpose::Index {
                    definition,
                    physical: index.data_number(),
                    usage: index.usage_map(),
                },
                Purpose::IndexBitmap {
                    definition,
                    physical: index.data_number(),
                    usage: index.usage_map(),
                },
            ],
            budget,
            cancelled,
        )?;
        Ok(metadata)
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
        let actual_count = (length - self.length) / PAGE_BYTES;
        let pending = actual_count != purposes.len();
        let bitmap = if pending {
            Some(self.global_bitmap().ok_or(EditError::Preservation)?.number)
        } else {
            None
        };
        if pending
            && bitmap
                != Some(
                    number
                        .max(global::INLINE_PAGES)
                        .checked_add(1)
                        .ok_or(InspectError::StructureLimit)?,
                )
        {
            return Err(EditError::Preservation);
        }
        budget
            .charge_allocation(actual_count as u64, size_of::<Reservation>() as u64)
            .map_err(InspectError::from)?;
        self.reservations
            .try_reserve(actual_count)
            .map_err(|_| InspectError::Allocation)?;
        let count = u32::try_from(purposes.len()).map_err(|_| InspectError::StructureLimit)?;
        number
            .checked_add(count)
            .ok_or(InspectError::StructureLimit)?;

        // Prepay cancellation before committing either member of a pair.
        for _ in 0..actual_count {
            check(budget, cancelled)?;
        }

        let mut next = number;
        for purpose in purposes {
            let purpose = match purpose {
                Purpose::Data => Purpose::Data,
                Purpose::GlobalBitmap { reference, block } => Purpose::GlobalBitmap {
                    reference: *reference,
                    block: *block,
                },
                Purpose::LongValueMapContainer { definition, column } => {
                    Purpose::LongValueMapContainer {
                        definition: *definition,
                        column: *column,
                    }
                }
                Purpose::LongValue {
                    column,
                    owned,
                    candidate,
                } => Purpose::LongValue {
                    column: *column,
                    owned: *owned,
                    candidate: *candidate,
                },
                Purpose::LongValueOwnedBitmap {
                    column,
                    owned,
                    block,
                } => Purpose::LongValueOwnedBitmap {
                    column: *column,
                    owned: *owned,
                    block: *block,
                },
                Purpose::OrdinaryOwnedBitmap {
                    definition,
                    reference,
                    block,
                } => Purpose::OrdinaryOwnedBitmap {
                    definition: *definition,
                    reference: *reference,
                    block: *block,
                },
                Purpose::OrdinaryCandidateBitmap {
                    definition,
                    reference,
                } => Purpose::OrdinaryCandidateBitmap {
                    definition: *definition,
                    reference: *reference,
                },
                Purpose::CreatedIndexMapContainer {
                    definition,
                    physical,
                } => Purpose::CreatedIndexMapContainer {
                    definition: *definition,
                    physical: *physical,
                },
                Purpose::CreatedIndexBody {
                    definition,
                    physical,
                    usage,
                } => Purpose::CreatedIndexBody {
                    definition: *definition,
                    physical: *physical,
                    usage: *usage,
                },
                Purpose::Index {
                    definition,
                    physical,
                    usage,
                } => Purpose::Index {
                    definition: *definition,
                    physical: *physical,
                    usage: *usage,
                },
                Purpose::IndexBitmap {
                    definition,
                    physical,
                    usage,
                } => Purpose::IndexBitmap {
                    definition: *definition,
                    physical: *physical,
                    usage: *usage,
                },
            };
            self.reservations.push(Reservation {
                number: next,
                purpose,
            });
            next = next.checked_add(1).ok_or(InspectError::StructureLimit)?;
            if bitmap == Some(next) {
                self.reservations.push(Reservation {
                    number: next,
                    purpose: Purpose::GlobalBitmap {
                        reference: GLOBAL_FREE_REFERENCE,
                        block: 0,
                    },
                });
                next = next.checked_add(1).ok_or(InspectError::StructureLimit)?;
            }
        }
        if next as usize * PAGE_BYTES != length {
            return Err(EditError::Preservation);
        }
        if pending {
            self.global_reserved = true;
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

    pub(in crate::table) fn require_global_bitmap(
        &self,
        number: u32,
        reference: u32,
        block: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        match self.reservation(number, budget, cancelled)?.purpose {
            Purpose::GlobalBitmap {
                reference: actual,
                block: actual_block,
            } if actual == reference && actual_block == block => Ok(()),
            _ => Err(EditError::Preservation),
        }
    }

    pub(in crate::table) fn global_bitmaps(&self) -> impl Iterator<Item = (u32, u32, u32)> + '_ {
        self.reservations
            .iter()
            .filter_map(|reservation| match reservation.purpose {
                Purpose::GlobalBitmap { reference, block } => {
                    Some((reservation.number, reference, block))
                }
                _ => None,
            })
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

    pub(in crate::table) fn require_long_value(
        &self,
        number: u32,
        column: usize,
        owned: u32,
        candidate: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        match self.reservation(number, budget, cancelled)?.purpose {
            Purpose::LongValue {
                column: actual_column,
                owned: actual_owned,
                candidate: actual_candidate,
            } if actual_column == column
                && actual_owned == owned
                && actual_candidate == candidate =>
            {
                Ok(())
            }
            _ => Err(EditError::Preservation),
        }
    }

    pub(in crate::table) fn require_index_bitmap(
        &self,
        number: u32,
        physical: u32,
        usage: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.require_index_bitmap_for(
            number,
            self.owner.single()?,
            physical,
            usage,
            budget,
            cancelled,
        )
    }

    pub(in crate::table) fn require_index_bitmap_for(
        &self,
        number: u32,
        definition: u32,
        physical: u32,
        usage: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        match self.reservation(number, budget, cancelled)?.purpose {
            Purpose::IndexBitmap {
                definition: actual_definition,
                physical: expected,
                usage: reference,
            } if actual_definition == definition && expected == physical && reference == usage => {
                Ok(())
            }
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

    pub(in crate::table) fn require_long_value_owned_bitmap(
        &self,
        number: u32,
        column: usize,
        owned: u32,
        block: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        match self.reservation(number, budget, cancelled)?.purpose {
            Purpose::LongValueOwnedBitmap {
                column: actual_column,
                owned: actual_owned,
                block: actual_block,
            } if actual_column == column && actual_owned == owned && actual_block == block => {
                Ok(())
            }
            _ => Err(EditError::Preservation),
        }
    }

    pub(in crate::table) fn long_value_owned_bitmaps(
        &self,
    ) -> impl Iterator<Item = LongValueOwnedBitmapReservation> + '_ {
        self.reservations
            .iter()
            .filter_map(|reservation| match reservation.purpose {
                Purpose::LongValueOwnedBitmap {
                    column,
                    owned,
                    block,
                } => Some(LongValueOwnedBitmapReservation {
                    number: reservation.number,
                    column,
                    owned,
                    block,
                }),
                _ => None,
            })
    }

    pub(in crate::table) fn long_values(&self) -> impl Iterator<Item = LongValueReservation> + '_ {
        self.reservations
            .iter()
            .filter_map(|reservation| match reservation.purpose {
                Purpose::LongValue {
                    column,
                    owned,
                    candidate,
                } => Some(LongValueReservation {
                    number: reservation.number,
                    column,
                    owned,
                    candidate,
                }),
                _ => None,
            })
    }

    pub(in crate::table) fn indexes(&self) -> impl Iterator<Item = IndexReservation> + '_ {
        self.reservations
            .iter()
            .filter_map(|reservation| match reservation.purpose {
                Purpose::Index {
                    definition,
                    physical,
                    usage,
                } => Some(IndexReservation {
                    definition,
                    number: reservation.number,
                    physical,
                    usage,
                }),
                _ => None,
            })
    }
}

impl State {
    pub(in crate::table::mutation) fn retain_source_metadata(
        &mut self,
        image: &NativeImage,
        metadata: Metadata,
    ) -> Result<(), EditError> {
        metadata.require_source(image, self.owner)?;
        if self.source_metadata.is_some() {
            return Err(EditError::Preservation);
        }

        self.source_metadata = Some(metadata);
        Ok(())
    }

    pub(in crate::table) fn reserve_long_value_map_container(
        &mut self,
        number: u32,
        column: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.commit(
            number,
            &[Purpose::LongValueMapContainer {
                definition: self.owner,
                column,
            }],
            budget,
            cancelled,
        )
    }

    pub(in crate::table) fn reserve_data(
        &mut self,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.commit(number, &[Purpose::Data], budget, cancelled)
    }

    pub(in crate::table) fn reserve_long_value(
        &mut self,
        number: u32,
        column: usize,
        owned: u32,
        candidate: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.commit(
            number,
            &[Purpose::LongValue {
                column,
                owned,
                candidate,
            }],
            budget,
            cancelled,
        )
    }

    pub(in crate::table) fn reserve_long_value_with_owned_bitmaps(
        &mut self,
        number: u32,
        column: usize,
        references: LongValueMapReferences,
        blocks: &[u32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u32>, EditError> {
        let count = blocks
            .len()
            .checked_add(1)
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge_allocation(count as u64, size_of::<Purpose>() as u64)
            .map_err(InspectError::from)?;
        let mut purposes = Vec::new();
        purposes
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        purposes.push(Purpose::LongValue {
            column,
            owned: references.owned(),
            candidate: references.candidate(),
        });
        let mut previous = None;
        for &block in blocks {
            check(budget, cancelled)?;
            if previous.is_some_and(|previous| previous >= block) {
                return Err(EditError::Preservation);
            }
            previous = Some(block);
            purposes.push(Purpose::LongValueOwnedBitmap {
                column,
                owned: references.owned(),
                block,
            });
        }
        budget
            .charge_allocation(count as u64, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        let mut numbers = Vec::new();
        numbers
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        for position in 0..count {
            check(budget, cancelled)?;
            numbers.push(self.planned_number(position)?);
        }
        self.commit(number, &purposes, budget, cancelled)?;
        Ok(numbers)
    }

    pub(in crate::table) fn reserve_owned_bitmap(
        &mut self,
        number: u32,
        reference: u32,
        block: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.commit(
            number,
            &[Purpose::OrdinaryOwnedBitmap {
                definition: self.owner,
                reference,
                block,
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
}

impl State<FreshIndexOwner> {
    pub(in crate::table) fn reserve_created_index_map(
        &mut self,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if number != self.owner.container() || self.reservations_len() != 0 {
            return Err(EditError::Preservation);
        }
        self.commit(
            number,
            &[Purpose::CreatedIndexMapContainer {
                definition: self.owner.definition(),
                physical: self.owner.physical(),
            }],
            budget,
            cancelled,
        )
    }

    pub(in crate::table) fn reserve_created_index_body(
        &mut self,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.owner.require_body(number)?;
        self.commit(
            number,
            &[Purpose::CreatedIndexBody {
                definition: self.owner.definition(),
                physical: self.owner.physical(),
                usage: self.owner.usage(),
            }],
            budget,
            cancelled,
        )
    }

    pub(in crate::table) fn created_index_maps(
        &self,
    ) -> impl Iterator<Item = CreatedIndexMapReservation> + '_ {
        self.reservations
            .iter()
            .filter_map(|reservation| match reservation.purpose {
                Purpose::CreatedIndexMapContainer {
                    definition,
                    physical,
                } => Some(CreatedIndexMapReservation {
                    number: reservation.number,
                    definition,
                    physical,
                }),
                _ => None,
            })
    }

    pub(in crate::table) fn created_indexes(&self) -> impl Iterator<Item = IndexReservation> + '_ {
        self.reservations
            .iter()
            .filter_map(|reservation| match reservation.purpose {
                Purpose::CreatedIndexBody {
                    definition,
                    physical,
                    usage,
                } => Some(IndexReservation {
                    number: reservation.number,
                    definition,
                    physical,
                    usage,
                }),
                _ => None,
            })
    }
}
