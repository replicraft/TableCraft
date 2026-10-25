//! Conditional previews use ordinary eager expression behavior, never stored caches.
use super::*;

#[test]
fn conditional_previews_keep_natural_numeric_text_and_null_condition_results() {
    for profile in [5, 6] {
        for (expression, expected) in [
            ("IIf([Id]=1,[Id]*10,[Id]*100)", "Recomputed\n10\n200\n300\n"),
            (
                "IIf([Id]=1,[Id],'other')",
                "Recomputed\n1\n\"\"\"other\"\"\"\n\"\"\"other\"\"\"\n",
            ),
            (
                "IIf(Null,'unused','kept')",
                "Recomputed\n\"\"\"kept\"\"\"\n\"\"\"kept\"\"\"\n\"\"\"kept\"\"\"\n",
            ),
        ] {
            let mut bytes = calculated_fixture::database_with_expression(
                &[Some(999), None, Some(41)],
                expression,
            );
            bytes[FORMAT_OFFSET] = profile;
            let database = Database::new(bytes);
            let output = database.preview(&[]);

            assert_success(&output);
            assert_eq!(output.stdout, expected.as_bytes(), "{expression}");
        }
    }
}

#[test]
fn inactive_arithmetic_errors_remain_eager_and_buffered_but_empty_input_can_finish() {
    let expression = "IIf(True,[Id],1/([Id]-2))";
    let database = Database::new(calculated_fixture::database_with_expression(
        &[Some(999), Some(999), Some(999)],
        expression,
    ));
    let output = database.preview(&[]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("division by zero"));

    let empty = Database::new(calculated_fixture::database_with_expression(
        &[],
        expression,
    ));
    let output = empty.preview(&[]);

    assert_success(&output);
    assert_eq!(output.stdout, b"Recomputed\n");
}

#[test]
fn invalid_hidden_arguments_are_refused_before_corrupt_user_records() {
    for expression in [
        "IIf(True,[Id],Date())",
        "IIf(True,[Id],IsNull([Id]))",
        "IIf(True,[Id],[Missing])",
        "IIf(True,[Id],[Score])",
        "IIf(True,[Id])",
        "IIf(True,[Id],)",
        "IIf$(True,[Id],0)",
        "VBA.IIf(True,[Id],0)",
    ] {
        let mut bytes = calculated_fixture::database_with_expression(&[Some(999)], expression);
        let page = DATA_PAGE * PAGE_BYTES;
        let row = u16::from_le_bytes(
            bytes[page + FIRST_ROW_SLOT..page + FIRST_ROW_SLOT + 2]
                .try_into()
                .unwrap(),
        ) as usize;
        bytes[page + row..page + row + 2].copy_from_slice(&0_u16.to_le_bytes());
        let database = Database::new(bytes);
        let output = database.preview(&[]);

        assert!(!output.status.success(), "{expression}");
        assert!(output.stdout.is_empty(), "{expression}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("row-local"),
            "{expression}: {:?}",
            output
        );
    }
}
