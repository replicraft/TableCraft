//! Causal tests of the extracted global proof; table/sibling proofs remain separate.
use super::audit_global;
use crate::{
    EditError,
    table::indexes::mutation::{RetiredPage, RetirementTable},
    test_support::base as fixture,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE_BYTES: usize = 4096;
const RETIRED: [usize; 2] = [14, 15];
const GLOBAL_START: usize = 2 * PAGE_BYTES - 64;

fn images() -> (Vec<u8>, Vec<u8>) {
    let mut before = fixture::database();
    before.resize(16 * PAGE_BYTES, 0);
    before[4..20].copy_from_slice(b"Standard ACE DB\0");
    before[20] = 6;
    let mut global = vec![0; 69];
    for page in 16..512 {
        global[5 + page / 8] |= 1 << (page % 8);
    }
    before[PAGE_BYTES..2 * PAGE_BYTES].copy_from_slice(&support::data_page(1, &[(0, global)]));
    for page in RETIRED {
        let offset = page * PAGE_BYTES;
        before[offset..offset + PAGE_BYTES].fill(0xa7);
        before[offset..offset + 2].copy_from_slice(&[4, 1]);
        support::put32(&mut before, offset + 4, 10);
    }
    let mut after = before.clone();
    for page in RETIRED {
        after[page * PAGE_BYTES..page * PAGE_BYTES + 4].fill(0);
        after[GLOBAL_START + page / 8] |= 1 << (page % 8);
    }
    (before, after)
}

fn audit(before: &[u8], after: &[u8], duplicate: bool) -> Result<(), EditError> {
    let mut budget = ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000);
    let schema = crate::inspect_table_schema(
        &mut Cursor::new(before),
        CatalogScope::new(1),
        "Items",
        &mut budget,
        &|| false,
    )?;
    let retired = [
        RetiredPage {
            number: 14,
            physical: 0,
            root: 13,
            usage: 0,
        },
        RetiredPage {
            number: if duplicate { 14 } else { 15 },
            physical: 0,
            root: 13,
            usage: 0,
        },
    ];
    let tables = [RetirementTable {
        schema: &schema,
        indexes: &[],
        retired: &retired,
    }];
    audit_global(before, after, &tables, &mut budget, &|| false)
}

#[test]
fn global_union_requires_every_retired_bit_and_no_unrequested_free_bit() {
    let (before, after) = images();
    audit(&before, &after, false).unwrap();
    let byte = GLOBAL_START + RETIRED[0] / 8;
    let mut missing = after.clone();
    missing[byte] &= !(1 << (RETIRED[1] % 8));
    assert_eq!(
        audit(&before, &missing, false),
        Err(EditError::Preservation)
    );
    let mut extra = after.clone();
    extra[byte] |= 1 << 5;
    assert_eq!(audit(&before, &extra, false), Err(EditError::Preservation));
    let mut already_free = before.clone();
    already_free[byte] |= 1 << (RETIRED[0] % 8);
    assert_eq!(
        audit(&already_free, &after, false),
        Err(EditError::Preservation)
    );
    assert_eq!(audit(&before, &after, true), Err(EditError::Preservation));
}

#[test]
fn global_retirement_proof_rejects_nonzero_prefix_or_any_changed_residual_byte() {
    let (before, after) = images();
    for offset in [
        RETIRED[0] * PAGE_BYTES,
        RETIRED[1] * PAGE_BYTES + 4,
        16 * PAGE_BYTES - 1,
    ] {
        let mut changed = after.clone();
        changed[offset] ^= 1;
        assert_eq!(
            audit(&before, &changed, false),
            Err(EditError::Preservation)
        );
    }
}
