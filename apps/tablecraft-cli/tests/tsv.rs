mod support;

#[test]
fn standard_tsv_reader_recovers_display_cells_without_null_text_ambiguity() {
    let output = support::run(&[
        "--format",
        "tsv",
        "query",
        "eval",
        "SELECT NULL AS Missing, '' AS Empty, 'NULL' AS TextNull, '雪\tline\n\"quote' AS [Quoted Header]",
    ]);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(output.stderr.is_empty());
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(output.stdout.as_slice());
    assert_eq!(
        reader.headers().unwrap().iter().collect::<Vec<_>>(),
        &["Missing", "Empty", "TextNull", "Quoted Header"]
    );
    let records: Vec<_> = reader.records().map(Result::unwrap).collect();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].iter().collect::<Vec<_>>(),
        ["NULL", "\"\"", "\"NULL\"", "\"雪\\tline\\n\\\"quote\""]
    );
}
