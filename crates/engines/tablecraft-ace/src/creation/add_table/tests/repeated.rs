//! Repeated append proofs use independent populated BEFORE and whole AFTER bytes.
use super::{POOLS, SCOPE, budget, definition, image};
use crate::{AddTableError, InspectError, add_table_source_available, read_table};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_testkit::fixtures::access::canonical_repeated_add_table as fixtures;

const PAGE_BYTES: usize = 4096;

#[test]
fn repeated_additions_match_independent_images_through_the_last_bitmap_member() {
    for existing in 2..=18 {
        let fixture = fixtures::with_existing_tables(existing);
        let original = fixture.before.clone();
        let plan = image(&fixture.before)
            .plan_add_table(definition(&fixture.table), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), original);
        assert_eq!(
            plan.after_bytes(),
            fixture.after,
            "existing tables: {existing}"
        );
        // Every earlier user definition, DATA page, primary root and opaque gap
        // remains at the same physical address, including the two populated tables.
        assert_eq!(
            &plan.after_bytes()[7 * PAGE_BYTES..original.len()],
            &original[7 * PAGE_BYTES..]
        );
    }
}

#[test]
fn both_populated_tables_remain_readable_and_third_is_empty() {
    let fixture = fixtures::source();
    let plan = image(&fixture.before)
        .plan_add_table(definition(&fixture.table), &mut budget(), &|| false)
        .unwrap();
    let after = image(plan.after_bytes());
    for (name, expected) in ["Items", "More"].into_iter().zip(fixture.original_rows) {
        let observed =
            read_table(&mut after.reader(), SCOPE, name, &mut budget(), &|| false).unwrap();
        assert_eq!(observed.rows(), &[expected]);
    }
    let added = read_table(
        &mut after.reader(),
        SCOPE,
        &fixture.table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(added.rows().is_empty());
    assert_eq!(added.schema().definition_page(), 13);
}

#[test]
fn names_and_every_existing_table_ownership_are_requalified() {
    let fixture = fixtures::source();
    for collision in ["Items", "mOrE"] {
        assert!(matches!(
            image(&fixture.before).plan_add_table(definition(collision), &mut budget(), &|| false,),
            Err(AddTableError::DuplicateName)
        ));
    }
    let mut bad_owner = fixture.before.clone();
    bad_owner[11 * PAGE_BYTES + 4..11 * PAGE_BYTES + 8].copy_from_slice(&7_u32.to_le_bytes());
    assert!(matches!(
        image(&bad_owner).plan_add_table(definition(&fixture.table), &mut budget(), &|| false),
        Err(AddTableError::Inspect(InspectError::Malformed))
    ));
    assert!(!add_table_source_available(&bad_owner, SCOPE, &mut budget(), &|| false).unwrap());

    let mut bad_membership = fixture.before;
    // More's existing unique key is now inconsistent with its retained Id=1 row.
    bad_membership[12 * PAGE_BYTES + 484] = 2;
    assert!(
        !add_table_source_available(&bad_membership, SCOPE, &mut budget(), &|| false,).unwrap()
    );
}

#[test]
fn advisory_discovers_legacy_bitmap_capacity_and_propagates_control_errors() {
    let fixture = fixtures::source();
    assert!(add_table_source_available(&fixture.before, SCOPE, &mut budget(), &|| false).unwrap());
    let capacity = fixtures::capacity_source();
    assert!(!add_table_source_available(&capacity, SCOPE, &mut budget(), &|| false).unwrap());
    // Valid framing requires dialect discovery before legacy capacity is known.
    assert!(matches!(
        add_table_source_available(&capacity, SCOPE, &mut ResourceBudget::new(0, 0, 1), &|| {
            false
        }),
        Err(AddTableError::Inspect(InspectError::Limit(_)))
    ));
    assert!(matches!(
        add_table_source_available(&capacity, SCOPE, &mut ResourceBudget::new(0, 0, 0), &|| {
            false
        }),
        Err(AddTableError::Inspect(InspectError::Limit(_)))
    ));
    assert!(matches!(
        add_table_source_available(&capacity, SCOPE, &mut ResourceBudget::new(0, 0, 1), &|| {
            true
        }),
        Err(AddTableError::Inspect(InspectError::Cancelled))
    ));
}

#[test]
fn repeated_proof_protects_old_and_new_roles_at_their_derived_addresses() {
    let fixture = fixtures::source();
    let before = image(&fixture.before);
    let source = super::source::capture(&before, &fixture.table, &mut budget(), &|| false).unwrap();
    let declaration = definition(&fixture.table);
    for address in [
        10 * PAGE_BYTES + 3500,
        11 * PAGE_BYTES + 512,
        12 * PAGE_BYTES + 3500,
        13 * PAGE_BYTES + 3500,
        14 * PAGE_BYTES + 512,
        15 * PAGE_BYTES + 3500,
    ] {
        let mut forged = fixture.after.clone();
        forged[address] ^= 1;
        assert!(matches!(
            super::audit::verify(
                &before,
                &forged,
                &declaration,
                &source,
                &mut budget(),
                &|| false,
            ),
            Err(AddTableError::Preservation)
        ));
    }
}

#[test]
fn repeated_source_control_costs_remain_nonzero_in_every_pool() {
    let fixture = fixtures::source();
    let mut measured = budget();
    let initial = POOLS.map(|pool| measured.remaining(pool));
    assert!(add_table_source_available(&fixture.before, SCOPE, &mut measured, &|| false).unwrap());
    let used: [u64; 3] =
        std::array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
    assert!(used.iter().all(|cost| *cost > 0));
    assert!(
        add_table_source_available(
            &fixture.before,
            SCOPE,
            &mut ResourceBudget::new(used[0], used[1], used[2]),
            &|| false,
        )
        .unwrap()
    );
    for missing in 0..3 {
        let mut short = used;
        short[missing] -= 1;
        assert!(matches!(
            add_table_source_available(
                &fixture.before,
                SCOPE,
                &mut ResourceBudget::new(short[0], short[1], short[2]),
                &|| false,
            ),
            Err(AddTableError::Inspect(InspectError::Limit(_)))
                | Err(AddTableError::Create(crate::CreateDatabaseError::Inspect(
                    InspectError::Limit(_)
                )))
        ));
    }
    assert!(measured.remaining(Resource::Steps) < initial[2]);
}

#[test]
fn repeated_source_refuses_lval_complex_and_extended_layouts_before_row_interpretation() {
    let fixture = fixtures::source();
    // More.Name is the second physical descriptor: first descriptor at75,
    // physical width25. These raw mutations are independent format inputs.
    let descriptor = 10 * PAGE_BYTES + 100;
    for (kind, flags, extended, width) in [
        (12_u8, 2_u8, 0_u8, 0_u16), // Long Text/LVAL
        (11, 2, 0, 0),              // OLE/LVAL
        (4, 3, 8, 4),               // Complex-value foreign key
        (10, 2, 1, 40),             // Extended Unicode compression layout
    ] {
        let mut bytes = fixture.before.clone();
        bytes[descriptor] = kind;
        bytes[descriptor + 15] = flags;
        bytes[descriptor + 16] = extended;
        bytes[descriptor + 23..descriptor + 25].copy_from_slice(&width.to_le_bytes());
        let captured = image(&bytes);
        assert!(matches!(
            super::source::qualify(&captured, &mut budget(), &|| false),
            Err(AddTableError::Source)
        ));
        assert_eq!(captured.as_bytes(), bytes);
        assert!(!add_table_source_available(&bytes, SCOPE, &mut budget(), &|| false).unwrap());
        assert!(matches!(
            captured.plan_add_table(definition(&fixture.table), &mut budget(), &|| false),
            Err(AddTableError::Source)
        ));
    }
    assert_eq!(fixture.before, fixtures::source().before);
}
