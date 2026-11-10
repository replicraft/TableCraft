use super::*;

fn complete(value: &str) -> Index {
    let mut index = Index::new(value);
    while !index.complete() {
        index.advance(value).unwrap();
    }
    index
}

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
        let index = complete(&value);
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
fn larger_unicode_and_blank_line_values_keep_sparse_indexes_and_reachable_suffixes() {
    let unicode = format!("{}tail", "🦀".repeat((9 * 1024 * 1024 - 4) / 4));
    let index = complete(&unicode);
    assert_eq!(unicode.len(), 9 * 1024 * 1024);
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

    let blank_lines = format!("{}終", "\n".repeat(9 * 1024 * 1024 - 3));
    let index = complete(&blank_lines);
    assert_eq!(blank_lines.len(), 9 * 1024 * 1024);
    assert_eq!(index.checkpoints.len(), index.row_count().div_ceil(32));
    let last = index
        .visible(&blank_lines, index.row_count() - 1..index.row_count())
        .unwrap();
    assert_eq!(&blank_lines[last[0].clone()], "終");
    let longer = format!("{blank_lines}x");
    let longer_index = complete(&longer);
    let last = longer_index
        .visible(
            &longer,
            longer_index.row_count() - 1..longer_index.row_count(),
        )
        .unwrap();
    assert_eq!(&longer[last[0].clone()], "終x");
}

#[test]
fn unchanged_virtual_window_reuses_ranges_and_replacement_lengths_are_rejected() {
    let value = "line\r\n".repeat(1024);
    let index = complete(&value);
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

#[test]
fn opening_allocates_no_checkpoints_and_each_frame_scans_only_its_actual_work() {
    let blank_lines = "\n".repeat(200_000);
    let mut index = Index::new(&blank_lines);
    assert_eq!(index.checkpoints.capacity(), 0);
    assert_eq!(index.row_count(), 0);
    index.advance(&blank_lines).unwrap();
    assert_eq!(index.row_count(), 8192);
    assert_eq!(index.cursor, 8192);
    assert!(!index.complete());
    assert!(index.checkpoints.capacity() <= 8192);

    let text = "🦀".repeat(200_000);
    let mut index = Index::new(&text);
    index.advance(&text).unwrap();
    assert!(index.cursor <= 64 * 1024 + 4096);
    assert!(text.is_char_boundary(index.cursor));
    assert!(!index.complete());
    let rows = index.row_count();
    assert_eq!(index.advance("changed"), Err(IndexError::Changed));
    assert_eq!(index.row_count(), rows);
}
