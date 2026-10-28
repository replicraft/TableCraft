//! Same-width native SID remasking, without activating database content.
use super::{inspect_table_indexes, record_layout};
use crate::{
    InspectError, NativeReadSource,
    format::{
        allocation,
        bytes::{u16_at, u32_at},
        data_type,
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        reader::{Reader, row_count},
        row::{DELETED_MASK, MASK_BITS, slot_offset},
    },
    header::check,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_crypto::Rc4Stream;
use tablecraft_model::CatalogScope;
use zeroize::Zeroizing;

const SID_COLUMNS: [(&str, &str); 2] = [("MSysACEs", "SID"), ("MSysObjects", "Owner")];

struct Patch {
    range: Range<usize>,
    before: Zeroizing<Vec<u8>>,
    after: Zeroizing<Vec<u8>>,
}

pub(crate) fn remask<R: NativeReadSource>(
    source: &mut R,
    old: &[u8; 4],
    new: &[u8; 4],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SecurityPatches, InspectError> {
    let _span = tracing::debug_span!("native.encryption.remask").entered();
    check(budget, cancelled)?;
    if old == new {
        return Ok(SecurityPatches(Vec::new()));
    }
    let mut patches = Vec::new();
    for (table, column) in SID_COLUMNS {
        let inspection =
            inspect_table_indexes(source, CatalogScope::new(1), table, budget, cancelled)?;
        let schema = inspection.schema();
        let column = schema
            .columns
            .iter()
            .find(|c| c.name() == column)
            .ok_or(InspectError::UnknownColumn)?;
        let field = &column.field;
        if column.scalar_type() != Some(tablecraft_schema::ScalarType::Binary)
            || field.kind != data_type::SHORT_BINARY
            || field.fixed
            || inspection
                .indexes()
                .iter()
                .any(|i| i.columns().iter().any(|c| c.number() == field.number))
        {
            return Err(InspectError::Unsupported);
        }
        let mut reader = Reader {
            source,
            header: schema.header,
            budget,
            cancelled,
            reads: 0,
        };
        let mut count = 0;
        for number in allocation::pages(&mut reader, schema.layout.usage)? {
            let page = reader.page(number)?;
            if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
                || u32_at(&page, OWNER_OFFSET)? != schema.definition
            {
                return Err(InspectError::Malformed);
            }
            for slot in 0..row_count(&page)? {
                reader.tick()?;
                if u16_at(&page, slot_offset(slot))? & DELETED_MASK != 0 {
                    continue;
                }
                reader.budget.charge(Resource::Rows, 1)?;
                count += 1;
                let row = reader.row_location(number, slot as u8, schema.definition, &page)?;
                let data = Zeroizing::new(row.data);
                let layout = record_layout::parse(&data, schema, reader.budget)?;
                if field.number >= layout.count
                    || data[layout.mask_start + field.number / MASK_BITS]
                        & (1 << (field.number % MASK_BITS))
                        == 0
                {
                    continue;
                }
                let start = *layout
                    .offsets
                    .get(field.variable)
                    .ok_or(InspectError::Unsupported)?;
                let end = *layout
                    .offsets
                    .get(field.variable + 1)
                    .ok_or(InspectError::Unsupported)?;
                if end - start > field.length {
                    return Err(InspectError::Malformed);
                }
                if start == end {
                    continue;
                }
                reader
                    .budget
                    .charge_allocation(2 * (end - start) as u64, 1)?;
                reader
                    .budget
                    .charge_allocation(1, size_of::<Patch>() as u64)?;
                let before = copy(&data[start..end])?;
                let mut after = copy(&before)?;
                for key in [old, new] {
                    Rc4Stream::new(key)
                        .map_err(|_| InspectError::MalformedEncryption)?
                        .apply(after.as_mut_slice());
                }
                patches
                    .try_reserve(1)
                    .map_err(|_| InspectError::Allocation)?;
                patches.push(Patch {
                    range: row.offset + start..row.offset + end,
                    before,
                    after,
                });
            }
        }
        if count != schema.declared_rows() {
            return Err(InspectError::Malformed);
        }
    }
    budget.charge(
        Resource::Steps,
        patches
            .len()
            .saturating_mul(patches.len().checked_ilog2().unwrap_or(0) as usize + 1) as u64,
    )?;
    patches.sort_unstable_by_key(|p| p.range.start);
    if patches
        .windows(2)
        .any(|p| p[0].range.end > p[1].range.start)
    {
        return Err(InspectError::Malformed);
    }
    Ok(SecurityPatches(patches))
}

pub(crate) struct SecurityPatches(Vec<Patch>);
impl SecurityPatches {
    pub(crate) fn into_pages(self) -> SecurityPages {
        SecurityPages {
            patches: self.0,
            next: 0,
        }
    }

    pub(crate) fn apply(
        self,
        image: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        let patches = self.0;
        for patch in &patches {
            check(budget, cancelled)?;
            if image.get(patch.range.clone()) != Some(patch.before.as_slice()) {
                return Err(InspectError::Malformed);
            }
        }
        // Caller owns an unpublished candidate; any later failure drops that image.
        for patch in patches {
            check(budget, cancelled)?;
            image[patch.range].copy_from_slice(&patch.after);
        }
        Ok(())
    }
}

fn copy(bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, InspectError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(bytes.len())
        .map_err(|_| InspectError::Allocation)?;
    result.extend_from_slice(bytes);
    Ok(Zeroizing::new(result))
}

// Sorted patches are checked against the original decoded page before mutation.
// A caller must discard its unpublished output after any error.
pub(crate) struct SecurityPages {
    patches: Vec<Patch>,
    next: usize,
}

impl SecurityPages {
    pub(crate) fn apply(
        &mut self,
        offset: usize,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        check(budget, cancelled)?;
        let end = offset
            .checked_add(page.len())
            .ok_or(InspectError::Malformed)?;
        let mut next = self.next;

        while let Some(patch) = self.patches.get(next) {
            check(budget, cancelled)?;
            if patch.range.start >= end {
                break;
            }
            if patch.range.start < offset || patch.range.end > end {
                return Err(InspectError::Malformed);
            }
            let range = patch.range.start - offset..patch.range.end - offset;
            if page.get(range) != Some(patch.before.as_slice()) {
                return Err(InspectError::Malformed);
            }
            next += 1;
        }

        for patch in &self.patches[self.next..next] {
            check(budget, cancelled)?;
            let range = patch.range.start - offset..patch.range.end - offset;
            page[range].copy_from_slice(&patch.after);
        }
        self.next = next;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<(), InspectError> {
        if self.next != self.patches.len() {
            return Err(InspectError::Malformed);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patches() -> SecurityPatches {
        SecurityPatches(vec![
            Patch {
                range: 18..20,
                before: Zeroizing::new(vec![2, 3]),
                after: Zeroizing::new(vec![8, 9]),
            },
            Patch {
                range: 25..26,
                before: Zeroizing::new(vec![9]),
                after: Zeroizing::new(vec![7]),
            },
        ])
    }

    fn budget() -> ResourceBudget {
        ResourceBudget::new(1024, 100, 1000)
    }

    #[test]
    fn page_patches_match_image_application_and_require_complete_consumption() {
        let original: Vec<u8> = (0..32).map(|value| value % 16).collect();
        let mut expected = original.clone();
        patches()
            .apply(&mut expected, &mut budget(), &|| false)
            .unwrap();

        let mut pages = patches().into_pages();
        let mut actual = original;
        for (number, page) in actual.chunks_mut(8).enumerate() {
            pages
                .apply(number * 8, page, &mut budget(), &|| false)
                .unwrap();
        }
        pages.finish().unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            patches().into_pages().finish(),
            Err(InspectError::Malformed)
        );
    }

    #[test]
    fn invalid_page_ranges_and_before_bytes_refuse_without_mutating_page() {
        for (offset, mut page) in [
            (19, vec![3, 4, 5]),
            (16, vec![0, 1, 2]),
            (16, vec![0, 1, 99, 3]),
        ] {
            let original = page.clone();
            let mut pages = patches().into_pages();
            assert_eq!(
                pages.apply(offset, &mut page, &mut budget(), &|| false),
                Err(InspectError::Malformed),
            );
            assert_eq!(page, original);
        }
    }

    #[test]
    fn all_before_images_on_a_page_are_checked_before_first_patch() {
        let mut pages = patches().into_pages();
        let mut page: Vec<u8> = (0..16).collect();
        page[9] = 99;
        let original = page.clone();
        assert_eq!(
            pages.apply(16, &mut page, &mut budget(), &|| false),
            Err(InspectError::Malformed),
        );
        assert_eq!(page, original);
    }
}
