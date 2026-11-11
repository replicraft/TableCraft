//! Literal zero-request groups retain a distinct original action and source binding.
#[cfg(feature = "ace16-create")]
mod recovery;

use crate::{
    FixedScalarUpdate, NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates,
    NativeUpdateIntent, NativeUpdateProfile,
};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 512 * 1024 * 1024)
}

fn groups() -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new_retained(
            "Score",
            NativeFittedUpdates::Fixed(Vec::new()),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn literal(before: &[u8], table: &str, names: &[&str]) -> Vec<u8> {
    let total = 72 + table.len() + names.iter().map(|name| 20 + name.len()).sum::<usize>();
    let mut wire = b"TCNI0024".to_vec();
    wire.extend((total as u32).to_le_bytes());
    wire.extend([24, 0, 0, 0]);
    wire.extend((before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    wire.extend(hash.finalize());
    for word in [table.len() as u32, names.len() as u32, 0, 0] {
        wire.extend(word.to_le_bytes());
    }
    wire.extend(table.as_bytes());
    for name in names {
        for word in [20 + name.len() as u32, name.len() as u32, 0, 0, 0] {
            wire.extend(word.to_le_bytes());
        }
        wire.extend(name.as_bytes());
    }
    wire
}

#[test]
fn empty_original_is_literal_bound_and_old_action_decoders_stay_closed() {
    let before = [0x24; 4096];
    let wire = literal(&before, "Table1", &["Score"]);
    let original = NativeFittedEditIntent::new_retained_empty(
        "Table1",
        groups(),
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        original
            .encode_retained_empty(&mut budget(), &|| false)
            .unwrap(),
        wire
    );
    let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
    assert_eq!(
        decoded.profile(),
        NativeUpdateProfile::Ace16RetainedEmptyRows
    );
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    let mut stale = before;
    stale[3000] ^= 1;
    assert!(
        decoded
            .verify_before(&stale, &mut budget(), &|| false)
            .is_err()
    );
    for decode in [
        NativeFittedEditIntent::decode_retained,
        NativeFittedEditIntent::decode_retained_opaque,
        NativeFittedEditIntent::decode_retained_currency,
        NativeFittedEditIntent::decode_retained_boolean,
        NativeFittedEditIntent::decode_retained_fixed_scalars,
        NativeFittedEditIntent::decode_retained_wide_values,
        NativeFittedEditIntent::decode_retained_boolean_materialization,
    ] {
        assert!(decode(&wire, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn empty_grammar_requires_named_unique_fixed_groups_and_no_cells() {
    let before = [0x24; 4096];
    for updates in [
        NativeFittedUpdates::Variable(Vec::new()),
        NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
            0,
            Value::Int32(17),
            Value::Int32(25),
        )]),
    ] {
        let group =
            NativeFittedIntentGroup::new_retained("Score", updates, &mut budget(), &|| false)
                .unwrap();
        assert!(
            NativeFittedEditIntent::new_retained_empty(
                "Table1",
                vec![group],
                &before,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    assert!(
        NativeFittedEditIntent::new_retained_empty(
            "Table1",
            Vec::new(),
            &before,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    for names in [vec![], vec![""], vec!["Score", "sCoRe"]] {
        assert!(
            NativeFittedEditIntent::decode_retained_empty(
                &literal(&before, "Table1", &names),
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    let wire = literal(&before, "Table1", &["Score"]);
    for end in 0..wire.len() {
        assert!(
            NativeFittedEditIntent::decode_retained_empty(&wire[..end], &mut budget(), &|| false)
                .is_err()
        );
    }
    for (offset, value) in [
        (0, b'X'),
        (8, 0),
        (12, 23),
        (14, 1),
        (60, 0),
        (64, 1),
        (68, 1),
        (78 + 8, 1),
        (78 + 12, 1),
        (78 + 16, 1),
    ] {
        let mut malformed = wire.to_vec();
        malformed[offset] = value;
        assert!(
            NativeFittedEditIntent::decode_retained_empty(&malformed, &mut budget(), &|| false)
                .is_err(),
            "offset={offset}"
        );
    }
}

#[test]
fn empty_data_decode_retains_cancellation_and_paid_allocation_failures() {
    let before = [0x24; 4096];
    let wire = literal(&before, "Table1", &["Score"]);
    assert!(NativeFittedEditIntent::decode_retained_empty(&wire, &mut budget(), &|| true).is_err());
    assert!(
        NativeFittedEditIntent::decode_retained_empty(
            &wire,
            &mut ResourceBudget::new(0, 100, 100_000),
            &|| false
        )
        .is_err()
    );
    assert!(
        NativeFittedEditIntent::decode_retained_empty(
            &wire,
            &mut ResourceBudget::new(100_000, 100, 0),
            &|| false
        )
        .is_err()
    );
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_empty_original_decodes_but_native_recovery_refuses_before_effects() {
    use crate::{EditError, NativeRecoveryQualificationError, WriteBlocker};
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;

    let before = vec![0x24; 4096];
    let wire = literal(&before, "Table1", &["Score"]);
    let original = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
    assert_eq!(
        original.profile(),
        NativeUpdateProfile::Ace16RetainedEmptyRows
    );
    let mut target = MemoryStorage::new(before.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal_file,
            &before,
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        original.qualify_retained_recovery(
            checked,
            CatalogScope::new(30601),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(WriteBlocker::RowLayout)
        ))
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
