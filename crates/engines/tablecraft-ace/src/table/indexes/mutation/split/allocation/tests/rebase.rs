//! Boundary decisions use complete original metadata and real EOF reservations.
use super::super::super::{AllocationPolicy, Plan};
use super::super::reserve;
use crate::{
    EditError, IndexCheckStatus, NativeImage, WriteBlocker, check_table,
    format::reader::Reader,
    table::mutation::append::State,
    test_support::{encoded::model, map_growth, support},
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const ROOT: usize = 95;
const MAP: usize = 14;
const GLOBAL_ROW: usize = 1;
const MAP_BYTES: usize = 69;
const BITMAP_PREFIX: usize = 4;
const BITMAP_CAPACITY: usize = (PAGE - BITMAP_PREFIX) * 8;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn source(eof: usize) -> Vec<u8> {
    assert!(eof >= 583);
    let mut bytes = map_growth::model::source(5).bytes;
    let old_root = bytes[13 * PAGE..14 * PAGE].to_vec();
    map_growth::model::install(&mut bytes, ROOT, &old_root);
    let usage = map_growth::model::selected_usage_position(&bytes, 0);
    support::put32(&mut bytes, usage + 4, ROOT as u32);
    map_growth::model::install(
        &mut bytes,
        MAP,
        &support::data_page(0, &[(0, map_growth::model::map(&[ROOT]))]),
    );

    bytes.resize(eof * PAGE, 0);
    let bitmap = eof - 1;
    let mut row = vec![0; MAP_BYTES];
    row[0] = 1;
    support::put32(&mut row, 1, bitmap as u32);
    map_growth::model::install(&mut bytes, GLOBAL_ROW, &support::data_page(1, &[(0, row)]));
    let mut block = vec![0; PAGE];
    block[..BITMAP_PREFIX].copy_from_slice(&[5, 1, 0, 0]);
    for number in eof..BITMAP_CAPACITY {
        block[BITMAP_PREFIX + number / 8] |= 1 << (number % 8);
    }
    map_growth::model::install(&mut bytes, bitmap, &block);

    let checked = check_table(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 900);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    assert_eq!(checked.indexes()[0].index().root_page(), ROOT as u32);

    bytes
}

fn append(image: &NativeImage, plan: &mut Plan) -> Result<u32, EditError> {
    let checked = check_table(
        &mut Cursor::new(image.as_bytes()),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let index = checked.indexes()[0].index();
    assert_eq!(index.usage_map(), (MAP as u32) << 8);
    let mut source = image.reader();
    let mut limits = budget();
    reserve(
        image,
        index,
        plan,
        &mut Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget: &mut limits,
            cancelled: &|| false,
            reads: 0,
        },
    )
}

fn rewritten(image: &NativeImage, plan: &Plan) -> Vec<u8> {
    let (start, end) = map_growth::model::row_range(image.as_bytes(), MAP, 0);
    assert_eq!(end - start, MAP_BYTES);
    let mut row = image.as_bytes()[start..end].to_vec();
    for edit in &plan.append.map_edits {
        if (start..end).contains(&edit.offset) {
            assert_eq!(image.as_bytes()[edit.offset], edit.before);
            row[edit.offset - start] = edit.after;
        }
    }
    row
}

fn inline(base: usize, members: &[usize]) -> Vec<u8> {
    let mut row = vec![0; MAP_BYTES];
    row[1..5].copy_from_slice(&(base as u32).to_le_bytes());
    for &member in members {
        let relative = member.checked_sub(base).unwrap();
        assert!(relative < 512);
        row[5 + relative / 8] |= 1 << (relative % 8);
    }
    row
}

#[test]
fn index_rebase_requires_strict_aligned_span_while_legacy_stays_closed() {
    for eof in [598, 599] {
        let bytes = source(eof);
        let image = model::capture(&bytes);
        let mut legacy = Plan::new(&image, 10);
        assert_eq!(
            append(&image, &mut legacy).err(),
            Some(EditError::Unsupported(WriteBlocker::RowLayout))
        );
        assert!(legacy.append.map_edits.is_empty());
        assert!(legacy.rebases.is_empty());
        assert!(legacy.promotions.is_empty());

        let mut qualified = Plan::with_policy(State::new(&image, 10), AllocationPolicy::Qualified);
        assert_eq!(append(&image, &mut qualified).unwrap(), eof as u32);
        if eof == 598 {
            assert_eq!(qualified.rebases.len(), 1);
            assert_eq!(qualified.rebases[0].base, 88);
            assert!(qualified.promotions.is_empty());
            assert_eq!(rewritten(&image, &qualified), inline(88, &[95, 598]));
            assert_eq!(qualified.append.next().unwrap(), 599);
        } else {
            assert!(qualified.rebases.is_empty());
            assert_eq!(qualified.promotions.len(), 1);
            let promoted = &qualified.promotions[0];
            assert_eq!(promoted.number, 600);
            for member in [95, 599] {
                assert_ne!(promoted.bitmap[4 + member / 8] & (1 << (member % 8)), 0,);
            }
            assert_eq!(qualified.append.next().unwrap(), 601);
        }
        assert_eq!(image.as_bytes(), bytes);
    }
}

#[test]
fn rebased_current_window_is_retained_until_later_promotion_absorbs_membership() {
    for original_base in [0, 88] {
        let mut bytes = source(598);
        map_growth::model::install(
            &mut bytes,
            MAP,
            &support::data_page(0, &[(0, inline(original_base, &[95]))]),
        );
        let image = model::capture(&bytes);
        let mut plan = Plan::with_policy(State::new(&image, 10), AllocationPolicy::Qualified);
        assert_eq!(append(&image, &mut plan).unwrap(), 598);
        assert_eq!(append(&image, &mut plan).unwrap(), 599);
        assert_eq!(rewritten(&image, &plan), inline(88, &[95, 598, 599]));
        assert!(plan.promotions.is_empty());
        if original_base == 0 {
            assert_eq!(plan.rebases[0].base, 88);
        } else {
            assert!(plan.rebases.is_empty());
        }

        assert_eq!(append(&image, &mut plan).unwrap(), 600);
        assert!(plan.rebases.is_empty());
        assert_eq!(plan.promotions.len(), 1);
        let promotion = &plan.promotions[0];
        assert_eq!(promotion.number, 601);
        let (start, end) = map_growth::model::row_range(&bytes, MAP, 0);
        assert!(
            plan.append
                .map_edits
                .iter()
                .all(|edit| !(start..end).contains(&edit.offset))
        );
        for member in [95, 598, 599, 600] {
            assert_ne!(promotion.bitmap[4 + member / 8] & (1 << (member % 8)), 0,);
        }
        assert_eq!(promotion.bitmap[4 + 601 / 8] & (1 << (601 % 8)), 0);
        assert_eq!(plan.append.next().unwrap(), 602);
        assert_eq!(image.as_bytes(), bytes);
    }
}
