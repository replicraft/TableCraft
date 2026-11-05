use super::*;

#[test]
fn sparse_rows_preserve_utf8_and_line_endings_without_extra_soft_wrap_lines() {
    for (value, expected) in [
        ("".to_owned(), vec!["".to_owned()]),
        (
            "a\r\nβ\u{2028}c\n".to_owned(),
            vec![
                "a".to_owned(),
                "β".to_owned(),
                "c".to_owned(),
                "".to_owned(),
            ],
        ),
        (
            format!("{}\nend", "x".repeat(1024)),
            vec!["x".repeat(1024), "end".to_owned()],
        ),
    ] {
        let index = Index::build(&value).unwrap();
        let visible = index.visible(&value, 0..index.row_count()).unwrap();
        let rows = visible
            .iter()
            .map(|range| &value[range.clone()])
            .collect::<Vec<_>>();
        assert_eq!(rows, expected);
        assert!(!index.continued_lines);
    }
}

#[test]
fn maximum_unicode_and_blank_line_values_have_bounded_sparse_indexes_and_reachable_suffixes() {
    let unicode = format!("{}tail", "🦀".repeat((8 * 1024 * 1024 - 4) / 4));
    let index = Index::build(&unicode).unwrap();
    assert_eq!(unicode.len(), 8 * 1024 * 1024);
    assert!(index.continued_lines);
    let last = index
        .visible(&unicode, index.row_count() - 2..index.row_count())
        .unwrap();
    let suffix = last
        .iter()
        .map(|range| &unicode[range.clone()])
        .collect::<String>();
    assert!(suffix.ends_with("tail"));
    assert!(
        last.iter()
            .all(|range| unicode[range.clone()].chars().count() <= 1024)
    );

    let blank_lines = format!("{}終", "\n".repeat(8 * 1024 * 1024 - 3));
    let index = Index::build(&blank_lines).unwrap();
    assert_eq!(blank_lines.len(), 8 * 1024 * 1024);
    assert!(index.checkpoints.capacity() * size_of::<u32>() <= 1024 * 1024 + 8);
    let last = index
        .visible(&blank_lines, index.row_count() - 1..index.row_count())
        .unwrap();
    assert_eq!(&blank_lines[last[0].clone()], "終");
    assert!(matches!(
        Index::build(&format!("{blank_lines}x")),
        Err(IndexError::Limit)
    ));
}

#[test]
fn unchanged_virtual_window_reuses_ranges_and_replacement_lengths_are_rejected() {
    let value = "line\r\n".repeat(1024);
    let index = Index::build(&value).unwrap();
    let mut visible = Visible::default();
    visible.update(&index, &value, 512..530).unwrap();
    let allocation = visible.rows.as_ptr();
    visible.update(&index, &value, 512..530).unwrap();
    assert_eq!(visible.rows.as_ptr(), allocation);
    assert!(
        visible
            .rows
            .iter()
            .all(|range| &value[range.clone()] == "line")
    );
    assert!(matches!(
        index.visible("replacement", 0..1),
        Err(IndexError::Changed)
    ));
}
