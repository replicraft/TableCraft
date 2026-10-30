//! A genuinely new Int32 physical population; no original NativeIndex is invented.
use super::{NumericKey, pages, profile::SingleProfile, repack};
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::{
        bytes::u16_at,
        reader::Reader,
        row::{OFFSET_MASK, reference},
    },
    header::check,
    table::{
        indexes::{
            key::KeyProfile,
            page::{Entry, KeyRef},
        },
        mutation::index_create::buffer,
        page::{self, TableSnapshotLimits},
        snapshot,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const WIRE_BYTES: usize = size_of::<u8>() + size_of::<i32>() + 2 * size_of::<u32>();
const SLOT_DIRECTORY_OFFSET: usize = 14;

pub(in crate::table) struct Population {
    keys: Vec<KeyRef<Option<i64>>>,
    profile: SingleProfile,
    middle: Option<usize>,
    distinct: u32,
}

pub(in crate::table) struct CreatedPage {
    pub number: u32,
    pub bytes: Vec<u8>,
}

impl Population {
    pub(in crate::table) fn capture(
        image: &NativeImage,
        checked: &TableCheck,
        column: usize,
        descending: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        let schema = checked.schema();
        budget
            .charge(Resource::Steps, schema.columns().len() as u64)
            .map_err(InspectError::from)?;
        let selected = schema
            .columns()
            .iter()
            .find(|field| field.number() == column)
            .ok_or(InspectError::UnknownColumn)?;
        let (positions, columns) =
            snapshot::projected_columns(schema, &[selected.name()], budget, cancelled)?;
        let records = page::read_records_batched(
            &mut image.reader(),
            schema,
            &columns,
            Some(&positions),
            TableSnapshotLimits::default(),
            budget,
            cancelled,
        )?;
        if records.len() != checked.rows() {
            return Err(EditError::Preservation);
        }
        let profile = SingleProfile {
            column,
            kind: NumericKey::new(ScalarType::Int32)?,
            descending,
        };
        let mut keys = buffer::vector(records.len(), budget, cancelled)?;
        let mut previous = None;
        for (logical, values) in records {
            check(budget, cancelled)?;
            let row = reference(logical.page_number(), logical.slot_number());
            if previous.is_some_and(|previous| previous >= row) {
                return Err(EditError::Preservation);
            }
            previous = Some(row);
            let mut input = image.reader();
            let mut reader = Reader {
                source: &mut input,
                header: image.catalog().header(),
                budget,
                cancelled,
                reads: 0,
            };
            let raw = reader.page(logical.page_number())?;
            let word = u16_at(
                &raw,
                SLOT_DIRECTORY_OFFSET + usize::from(logical.slot_number()) * size_of::<u16>(),
            )?;
            if word & !OFFSET_MASK != 0 {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let location = reader.row_location(
                logical.page_number(),
                logical.slot_number(),
                schema.definition_page(),
                &raw,
            )?;
            let record_bytes = size_of::<u16>()
                + schema.columns().len() * size_of::<i32>()
                + schema.columns().len().div_ceil(u8::BITS as usize);
            if location.data.len() != record_bytes {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let [value] = values.as_slice() else {
                return Err(EditError::Preservation);
            };
            let key = match value {
                Value::Null => None,
                Value::Int32(value) => Some(i64::from(*value)),
                _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
            };
            keys.push(KeyRef { key, row });
        }
        let levels = keys.len().checked_ilog2().unwrap_or(0) as usize + 1;
        let work = keys
            .len()
            .checked_mul(levels)
            .and_then(|value| value.checked_mul(profile.max_key_bytes()))
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge(Resource::Steps, work as u64)
            .map_err(InspectError::from)?;
        keys.sort_unstable_by(|left, right| left.compare_with(right, &profile));
        let mut distinct = 0_u32;
        let mut previous = None;
        for key in &keys {
            check(budget, cancelled)?;
            if previous != Some(key.key) {
                distinct = distinct
                    .checked_add(1)
                    .ok_or(InspectError::StructureLimit)?;
                previous = Some(key.key);
            }
        }
        let borrowed = borrowed(&keys, budget, cancelled)?;
        let middle = if repack::borrowed_fits_fresh::<_, WIRE_BYTES>(
            &borrowed, &profile, budget, cancelled,
        )? {
            None
        } else {
            let middle = keys.len() / 2;
            if middle == 0
                || !repack::borrowed_fits_fresh::<_, WIRE_BYTES>(
                    &borrowed[..middle],
                    &profile,
                    budget,
                    cancelled,
                )?
                || !repack::borrowed_fits_fresh::<_, WIRE_BYTES>(
                    &borrowed[middle..],
                    &profile,
                    budget,
                    cancelled,
                )?
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            Some(middle)
        };
        check(budget, cancelled)?;
        Ok(Self {
            keys,
            profile,
            middle,
            distinct,
        })
    }

    pub(in crate::table) fn rows(&self) -> usize {
        self.keys.len()
    }

    pub(in crate::table) fn distinct(&self) -> u32 {
        self.distinct
    }

    pub(in crate::table) fn body_pages(&self) -> usize {
        if self.middle.is_some() { 3 } else { 1 }
    }

    pub(in crate::table) fn build(
        &self,
        owner: u32,
        root: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<CreatedPage>, EditError> {
        let entries = borrowed(&self.keys, budget, cancelled)?;
        let mut output = buffer::vector(self.body_pages(), budget, cancelled)?;
        if let Some(middle) = self.middle {
            let left = root.checked_add(1).ok_or(InspectError::StructureLimit)?;
            let right = left.checked_add(1).ok_or(InspectError::StructureLimit)?;
            let maximum = &self.keys[middle - 1];
            let root_entries = [Entry {
                value: KeyRef {
                    key: &maximum.key,
                    row: maximum.row,
                },
                child: left,
            }];
            output.push(CreatedPage {
                number: root,
                bytes: pages::node::<_, WIRE_BYTES>(
                    &pages::NodeHeader {
                        owner,
                        height: 1,
                        links: (0, 0),
                        tail: right,
                    },
                    &root_entries,
                    &self.profile,
                    budget,
                    cancelled,
                )?,
            });
            output.push(CreatedPage {
                number: left,
                bytes: pages::leaf::<_, WIRE_BYTES>(
                    owner,
                    0,
                    right,
                    &entries[..middle],
                    &self.profile,
                    budget,
                    cancelled,
                )?,
            });
            output.push(CreatedPage {
                number: right,
                bytes: pages::leaf::<_, WIRE_BYTES>(
                    owner,
                    left,
                    0,
                    &entries[middle..],
                    &self.profile,
                    budget,
                    cancelled,
                )?,
            });
        } else {
            output.push(CreatedPage {
                number: root,
                bytes: pages::leaf::<_, WIRE_BYTES>(
                    owner,
                    0,
                    0,
                    &entries,
                    &self.profile,
                    budget,
                    cancelled,
                )?,
            });
        }
        Ok(output)
    }
}

fn borrowed<'a>(
    keys: &'a [KeyRef<Option<i64>>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Entry<&'a Option<i64>>>, InspectError> {
    let mut result = buffer::vector(keys.len(), budget, cancelled)?;
    for key in keys {
        check(budget, cancelled)?;
        result.push(Entry {
            value: KeyRef {
                key: &key.key,
                row: key.row,
            },
            child: 0,
        });
    }
    Ok(result)
}
