use super::*;

const FORMAT_OFFSET: usize = 20;
const PAGE_BYTES: usize = 4096;
const DATA_PAGE: usize = 12;
const FIRST_ROW_SLOT: usize = 14;
const CACHE_WRAPPER_OFFSET: usize = 6;

#[test]
fn opted_in_cached_streams_preserve_stale_values_without_running_formulas() {
    for profile in [5, 6] {
        let mut bytes =
            calculated_fixture::database_with_expression(&[Some(999), None, Some(41)], "1/0");
        bytes[FORMAT_OFFSET] = profile;
        let file = FileCopy::new(bytes);
        let sql = "SELECT Id,Score FROM Items";
        let expected = b"Id\tScore\n1\t999\n2\tNULL\n3\t41\n";

        assert!(failure(file.query(sql, &["--stream"])).is_empty());
        assert_eq!(
            success(file.query(sql, &["--stored-calculations"])),
            expected
        );
        for batch in ["1", "2"] {
            assert_eq!(
                success(file.query(
                    sql,
                    &["--stream", "--stored-calculations", "--batch-rows", batch],
                )),
                expected
            );
        }
    }
}

#[test]
fn cached_count_operands_filter_and_union_through_the_existing_evaluator() {
    let file = FileCopy::new(calculated_fixture::database(&[Some(999), None, Some(41)]));
    for (sql, expected) in [
        (
            "SELECT Count(*) AS N,Count(Score) AS Cached FROM Items",
            b"N\tCached\n3\t2\n".as_slice(),
        ),
        (
            "SELECT Id FROM Items WHERE Score>100 UNION ALL SELECT Id FROM Items WHERE Score IS NULL",
            b"Id\n1\n2\n".as_slice(),
        ),
    ] {
        assert_eq!(
            success(file.query(sql, &["--stored-calculations"])),
            expected
        );
        assert_eq!(
            success(file.query(
                sql,
                &["--stream", "--stored-calculations", "--batch-rows", "1"],
            )),
            expected
        );
    }
}

#[test]
fn corrupt_selected_cache_preserves_only_completed_batches_and_can_be_pruned() {
    let mut bytes = calculated_fixture::database(&[Some(999), None, Some(41)]);
    let page = DATA_PAGE * PAGE_BYTES;
    let slot = page + FIRST_ROW_SLOT + 2 * size_of::<u16>();
    let row = u16::from_le_bytes(bytes[slot..slot + size_of::<u16>()].try_into().unwrap());
    bytes[page + usize::from(row) + CACHE_WRAPPER_OFFSET] = 1;
    let file = FileCopy::new(bytes);
    let arguments = ["--stream", "--stored-calculations", "--batch-rows", "1"];

    assert_eq!(
        failure(file.query("SELECT Score FROM Items", &arguments)),
        b"Score\n999\nNULL\n"
    );
    assert!(failure(file.query("SELECT Score FROM Items", &["--stored-calculations"],)).is_empty());
    assert!(failure(file.query("SELECT Count(Score) FROM Items", &arguments)).is_empty());
    assert_eq!(
        success(file.query("SELECT Id FROM Items", &arguments)),
        b"Id\n1\n2\n3\n"
    );
    assert_eq!(
        success(file.query("SELECT Count(*) AS N FROM Items", &arguments)),
        b"N\n3\n"
    );
}
