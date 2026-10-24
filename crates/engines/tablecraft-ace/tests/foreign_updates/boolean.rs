use super::*;
use tablecraft_ace::{FixedColumnUpdates, RelationshipCheckStatus};

const TABLE_PAGE: usize = 10;
const DATA_PAGE: usize = 12;
const FIRST_FIELD: usize = 87;
const FIELD_BYTES: usize = 25;
const PHYSICAL_INDEX: usize = 155;
const PHYSICAL_BYTES: usize = 52;

fn database(code: u8) -> Vec<u8> {
    let mut bytes = fixture::database(code, false, true);
    let definition = &mut bytes[TABLE_PAGE * PAGE_BYTES..(TABLE_PAGE + 1) * PAGE_BYTES];
    support::put32(definition, 16, 2);
    definition[FIRST_FIELD] = 1;
    support::put16(definition, FIRST_FIELD + 21, 0);
    support::put16(definition, FIRST_FIELD + 23, 1);
    support::put16(definition, FIRST_FIELD + FIELD_BYTES + 21, 0);
    // Both reciprocal trees reference the Boolean Id field. Score remains an
    // unrelated ordinary Int32 so its edits cannot bypass relationship coverage.
    support::put16(definition, PHYSICAL_INDEX + PHYSICAL_BYTES + 4, 0);
    let rows = [(true, 10_i32), (false, 20)]
        .into_iter()
        .map(|(key, score)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(score.to_le_bytes());
            row.push(u8::from(key) | 2);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(TABLE_PAGE as u32, &rows));
    for page in [13, 16] {
        bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES].copy_from_slice(&index());
    }
    let mut relationship = 8_u16.to_le_bytes().to_vec();
    for value in [0x1100_u32, 1, 0] {
        relationship.extend(value.to_le_bytes());
    }
    let mut offsets = vec![relationship.len() as u16];
    for name in ["SelfLink", "Items", "Items", "Id", "Id"] {
        relationship.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
        offsets.push(relationship.len() as u16);
    }
    for offset in offsets.into_iter().rev() {
        relationship.extend(offset.to_le_bytes());
    }
    relationship.extend(5_u16.to_le_bytes());
    relationship.push(0xff);
    bytes[9 * PAGE_BYTES..10 * PAGE_BYTES]
        .copy_from_slice(&support::data_page(7, &[(0, relationship)]));
    bytes
}

fn index() -> Vec<u8> {
    let mut page = vec![0xa7; PAGE_BYTES];
    page[..480].fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    support::put32(&mut page, 4, TABLE_PAGE as u32);
    let mut end = 0;
    // Original native Boolean encoding: true sorts before false, with a
    // one-byte atom followed by the physical row reference.
    for (row, key) in [(0_u32, 0_u8), (1, 255)] {
        let mut wire = vec![127, key];
        wire.extend(((DATA_PAGE as u32) << 8 | row).to_be_bytes());
        page[480 + end..480 + end + wire.len()].copy_from_slice(&wire);
        end += wire.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(&mut page, 2, (PAGE_BYTES - 480 - end) as u16);
    page
}

#[test]
fn checked_boolean_relationships_still_block_fixed_batch_and_cascade_previews() {
    for code in [5, 6] {
        let bytes = database(code);
        let structural = check_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            structural
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        let report = check_table_constraints(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            report.relationships()[0].status(),
            RelationshipCheckStatus::Checked
        );

        for (column, old, new) in [
            ("Id", Value::Boolean(true), Value::Boolean(true)),
            ("Score", Value::Int32(10), Value::Int32(11)),
        ] {
            assert!(matches!(
                preview(&bytes, column, 0, old, new),
                Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
            ));
        }
        let assignments = [FixedScalarUpdate::new(
            0,
            Value::Int32(10),
            Value::Int32(11),
        )];
        let groups = [FixedColumnUpdates::new("Items", "Score", &assignments)];
        assert!(matches!(
            capture(&bytes).preview_ace16_batch_updates(&groups, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        ));
        assert!(matches!(
            capture(&bytes).preview_ace16_cascading_updates(&groups, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        ));
    }
}

#[test]
fn ordinary_jet4_writes_cannot_use_boolean_read_coverage_after_foreign_flags_are_removed() {
    let mut bytes = database(6);
    bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    bytes[20] = 1;
    // Preserve the catalog relationship and both physical key trees, but turn
    // reciprocal foreign aliases into ordinary indexes. The ordinary writer's
    // foreign-index blocker alone can no longer protect this shape.
    for number in [1, 2] {
        let logical = TABLE_PAGE * PAGE_BYTES + 259 + number * 28;
        bytes[logical + 12..logical + 24].fill(0);
    }
    let report = check_table_constraints(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        report.relationships()[0].status(),
        RelationshipCheckStatus::Checked
    );
    let structural = check_table(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(structural.indexes().iter().all(|index| {
        index.status() == IndexCheckStatus::Checked && index.index().native_type() != 2
    }));
    assert!(matches!(
        capture(&bytes).plan_int32_updates(
            "Items",
            "Score",
            &[Int32Update::new(0, 10, 11)],
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    ));

    // The same ordinary field/index shape remains writable when the catalog
    // explicitly disables enforcement. This isolates the new relationship
    // gate from other possible blockers and preserves NotEnforced behavior.
    let relationship_page = 9 * PAGE_BYTES;
    let row = u16::from_le_bytes(
        bytes[relationship_page + 14..relationship_page + 16]
            .try_into()
            .unwrap(),
    ) as usize;
    support::put32(&mut bytes, relationship_page + row + 2, 0x1102);
    let report = check_table_constraints(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        report.relationships()[0].status(),
        RelationshipCheckStatus::NotEnforced
    );
    let plan = capture(&bytes)
        .plan_int32_updates(
            "Items",
            "Score",
            &[Int32Update::new(0, 10, 11)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.changed_rows(), 1);
}
