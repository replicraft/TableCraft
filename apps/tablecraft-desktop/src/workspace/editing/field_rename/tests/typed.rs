//! Saved typed columns use real rename proposals and keep native cell identities.
use crate::workspace::editing::test_support::field_rename::{propose_name, rename_and_save};
use crate::workspace::{
    Desktop,
    test_support::{
        app_harness,
        cell_edit::{begin_replacement, review_and_save},
        finish_reads, finish_schema_save,
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    exact_values, field_changes, fields, fixed_scalars,
};

fn open_table(harness: &mut Harness<'_, Desktop>, table: &str) {
    harness.get_by_role_and_label(Role::Button, table).focus();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(1);
    finish_reads(harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().table, table);
}

#[test]
fn saved_byte_rename_keeps_other_fixed_values_and_the_renamed_cell_editable() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (before.len() / 4096) as u32;
    let catalog_root = usize::from(u16::from_le_bytes(
        before[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let created = fixed_scalars::append_table(&before, "Measurements");
    let values = [
        fixed_scalars::Scalar::Byte(17),
        fixed_scalars::Scalar::Int16(-1234),
        fixed_scalars::Scalar::Int64(9_007_199_254_740_993),
        fixed_scalars::Scalar::SingleBits(0x3fa0_0000),
        fixed_scalars::Scalar::DoubleBits(0x3ff3_c0ca_428c_59dd),
        fixed_scalars::Scalar::DateTimeBits(0x4004_0000_0000_0000),
    ];
    let mut expected = fixed_scalars::append_values(&created, definition, &values);
    let (_directory, path, _, mut harness) = opened(expected.clone());
    open_table(&mut harness, "Measurements");
    let tab = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &tab);
    let after = field_changes::rename(&expected, definition, catalog_root, "Score", "Reading");
    rename_and_save(&mut harness, &path, &expected, &after, "Score", "Reading");
    expected = after;
    begin_replacement(&mut harness, 0, "Reading", false, Some("124"));
    let after = fixed_scalars::update_score(&expected, definition, 0, Some(124));
    review_and_save(&mut harness, &path, &expected, &after);
    expected = after;
    let original = harness
        .state()
        .state
        .data
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| (cell.text.clone(), cell.kind))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let numbers = harness
        .state()
        .state
        .data
        .as_ref()
        .unwrap()
        .columns
        .iter()
        .map(|column| column.native_number)
        .collect::<Vec<_>>();
    let mut old = "ByteValue";
    for new in ["ByteMeasurementExpanded", "B", "b", "b"] {
        let after = field_changes::rename(&expected, definition, catalog_root, old, new);
        rename_and_save(&mut harness, &path, &expected, &after, old, new);
        expected = after;
        let data = harness.state().state.data.as_ref().unwrap();
        assert_eq!(data.columns[2].name, new);
        assert_eq!(
            data.columns
                .iter()
                .map(|column| column.native_number)
                .collect::<Vec<_>>(),
            numbers
        );
        for (row, prior) in data.rows.iter().zip(&original) {
            for (cell, prior) in row.iter().zip(prior) {
                assert_eq!(cell.text, prior.0);
                assert!(cell.kind == prior.1);
            }
        }
        assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
        old = new;
    }
    begin_replacement(&mut harness, 0, "b", false, Some("200"));
    let after = fixed_scalars::update(
        &expected,
        definition,
        0,
        fixed_scalars::Column::ByteValue,
        Some(fixed_scalars::Scalar::Byte(200)),
    );
    review_and_save(&mut harness, &path, &expected, &after);
    expected = after;
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    open_table(&mut fresh, "Measurements");
    let data = fresh.state().state.data.as_ref().unwrap();
    assert_eq!(data.columns[2].name, "b");
    assert_eq!(data.rows[0][1].text, "124");
    assert_eq!(data.rows[0][2].text, "200");
    assert_image(&path, &expected);
    assert!(!journal_path(&path).exists());
}

#[test]
fn saved_guid_and_decimal_rename_keep_exact_values_and_native_number_mapping() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (before.len() / 4096) as u32;
    let catalog_root = usize::from(u16::from_le_bytes(
        before[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let created = exact_values::append_table(&before, "ExactValues");
    let token = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ];
    let mut expected = exact_values::append(
        &created,
        definition,
        Some(token),
        Some(90_071_992_547_409_931_234),
    );
    let (_directory, path, _, mut harness) = opened(expected.clone());
    open_table(&mut harness, "ExactValues");
    let tab = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &tab);
    let after = field_changes::rename(&expected, definition, catalog_root, "Score", "Reading");
    rename_and_save(&mut harness, &path, &expected, &after, "Score", "Reading");
    expected = after;
    begin_replacement(&mut harness, 0, "Reading", false, Some("124"));
    let after = exact_values::update_score(&expected, definition, 0, Some(124));
    review_and_save(&mut harness, &path, &expected, &after);
    expected = after;
    for (index, names) in [
        (2, ["Token", "ExactIdentifierExpanded", "T", "t", "t"]),
        (3, ["Amount", "PreciseAmountExpanded", "A", "a", "a"]),
    ] {
        for pair in names.windows(2) {
            let after =
                field_changes::rename(&expected, definition, catalog_root, pair[0], pair[1]);
            rename_and_save(&mut harness, &path, &expected, &after, pair[0], pair[1]);
            expected = after;
            let data = harness.state().state.data.as_ref().unwrap();
            assert_eq!(data.columns[index].name, pair[1]);
            assert_eq!(data.columns[index].native_number, Some(index));
            assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
        }
    }
    begin_replacement(&mut harness, 0, "a", false, Some("-9007199254740993.4321"));
    let after =
        exact_values::update_decimal(&expected, definition, 0, Some(-90_071_992_547_409_934_321));
    review_and_save(&mut harness, &path, &expected, &after);
    expected = after;
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    open_table(&mut fresh, "ExactValues");
    let data = fresh.state().state.data.as_ref().unwrap();
    assert_eq!(data.columns[2].name, "t");
    assert_eq!(data.columns[3].name, "a");
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][1].text, "124");
    assert_image(&path, &expected);
    assert!(!journal_path(&path).exists());
}

fn assert_image(path: &std::path::Path, expected: &[u8]) {
    let actual = std::fs::read(path).unwrap();
    let difference = actual
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right);
    assert!(
        actual.len() == expected.len() && difference.is_none(),
        "complete typed-rename image lengths {} / {}, first mismatch {:?}",
        actual.len(),
        expected.len(),
        difference
    );
}

#[test]
fn typed_casefold_collision_keeps_proposal_and_source_until_corrected_review() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (before.len() / 4096) as u32;
    let catalog_root = usize::from(u16::from_le_bytes(
        before[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let created = exact_values::append_table(&before, "ExactValues");
    let source = exact_values::append_omitted(&created, definition);
    let expected = field_changes::rename(&source, definition, catalog_root, "Token", "Identifier");
    let (_directory, path, original, mut harness) = opened(source);
    open_table(&mut harness, "ExactValues");
    propose_name(&mut harness, "Token", "score");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while harness.state().state.busy || harness.state().editing.field_rename.is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "typed collision review stalled"
        );
        assert!(harness.state().editing.save.is_none());
        assert!(!harness.state().editing.worker.is_busy());
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert!(harness.state().state.error.is_some());
    let (target, proposed, rejected_revision) =
        harness.state().state.field_rename_proposal().unwrap();
    assert_eq!(target.column, "Token");
    assert_eq!(proposed, "score");
    assert_image(&path, &original);
    assert!(!journal_path(&path).exists());
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
    let input = harness.get_by_role_and_label(Role::TextInput, "Field name");
    assert_eq!(input.value().as_deref(), Some("score"));
    input.focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text("Identifier");
    harness.run_steps(2);
    let (_, corrected, revision) = harness.state().state.field_rename_proposal().unwrap();
    assert_eq!(corrected, "Identifier");
    assert!(revision > rejected_revision);
    assert!(harness.state().state.can_review_field_rename());
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    finish_schema_save(&mut harness);
    assert_image(&path, &expected);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().state.field_rename_open());
}

#[test]
fn keyless_first_data_column_renames_without_becoming_the_record_selector() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let catalog_root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let before = field_changes::keyless_first_field(&seed, "Keyless");
    let after = field_changes::rename(&before, definition, catalog_root, "Id", "Identifier");
    let (_directory, path, _, mut harness) = opened(before.clone());
    open_table(&mut harness, "Keyless");
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.columns[0].name, "Id");
    assert_eq!(data.columns[0].native_number, Some(0));
    assert_eq!(data.rows[0][0].text, "17");
    assert_eq!(data.rows[0][1].text, "23");
    let selectors = harness.query_all_by_role(Role::RowHeader).count();
    assert!(
        selectors > 0,
        "native field zero is separate from the actual record selector"
    );
    rename_and_save(&mut harness, &path, &before, &after, "Id", "Identifier");
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.columns[0].name, "Identifier");
    assert_eq!(data.columns[0].native_number, Some(0));
    assert_eq!(data.columns[1].name, "Score");
    assert_eq!(data.columns[1].native_number, Some(1));
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][0].text, "17");
    assert_eq!(data.rows[0][1].text, "23");
    assert_eq!(
        harness.query_all_by_role(Role::RowHeader).count(),
        selectors
    );
    assert_image(&path, &after);
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    open_table(&mut fresh, "Keyless");
    let data = fresh.state().state.data.as_ref().unwrap();
    assert_eq!(data.columns[0].name, "Identifier");
    assert_eq!(data.columns[0].native_number, Some(0));
    assert_eq!(data.rows[0][0].text, "17");
    assert_eq!(data.rows[0][1].text, "23");
    assert_image(&path, &after);
    assert!(!journal_path(&path).exists());
}
