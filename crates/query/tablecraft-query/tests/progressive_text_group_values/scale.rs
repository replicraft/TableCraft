use super::*;

struct Wide {
    columns: Vec<Column>,
    visits: Cell<usize>,
    eof: Cell<bool>,
}

impl Wide {
    fn new() -> Self {
        Self {
            columns: vec![
                Column::new("UnusedMemo".into(), ScalarType::Text, false).unwrap(),
                Column::new("Label".into(), ScalarType::Text, false).unwrap(),
                Column::new("UnusedBinary".into(), ScalarType::Binary, false).unwrap(),
            ],
            visits: Cell::new(0),
            eof: Cell::new(false),
        }
    }
}

impl Source for Wide {
    fn columns(&self, _table: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }

    fn scan(
        &self,
        _table: &str,
        _visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("large Text grouping must not materialize a source snapshot");
    }

    fn scan_projected_progressive(
        &self,
        _table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert_eq!(indices, [1]);
        self.eof.set(false);
        for ordinal in 0..100_003 {
            // One transient 8 KiB payload pair per generated row. Retaining
            // these unselected fields for all rows would exceed this test's
            // query allowance; the source never builds a population Vec.
            let row = [
                Value::Text("m".repeat(4096)),
                Value::Text(format!("bucket{:02}", ordinal % 17)),
                Value::Binary(vec![0xa7; 4096]),
            ];
            self.visits.set(self.visits.get() + 1);
            visitor(ProjectedRow::select(&row, indices)?)?;
        }
        self.eof.set(true);
        Ok(())
    }
}

#[test]
fn generated_wide_count_projects_only_text_keys_without_a_source_snapshot() {
    let source = Wide::new();
    let sql = "SELECT Label,Count(*) FROM T GROUP BY Label";
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare_with_options(syntax.query(), &source, Default::default()).unwrap();
    let mut actual = Vec::new();
    let summary = plan
        .execute_progressive(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&ORDINAL),
                limits: QueryLimits {
                    bytes: 192 * 1024 * 1024,
                    rows: 800_000,
                    steps: 100_000_000,
                },
                ..Default::default()
            },
            &mut |row| {
                assert!(source.eof.get());
                actual.push(row.to_vec());
                Ok::<_, ()>(())
            },
        )
        .unwrap();
    let expected: Vec<_> = (0..17)
        .map(|bucket| {
            let count = (100_002 - bucket) / 17 + 1;
            vec![
                Value::Text(format!("bucket{bucket:02}")),
                Value::Int64(count),
            ]
        })
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(summary.scanned_rows, 100_003);
    assert_eq!(summary.emitted_rows, 17);
    assert_eq!(source.visits.get(), 100_003);
}
