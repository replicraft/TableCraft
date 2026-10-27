use super::*;

#[test]
fn both_ace16_headers_count_all_forms_and_allow_owned_opaque_orphan_rows() {
    for code in [5, 6] {
        let bytes = model::database(code);
        let before = bytes.clone();
        readable(&bytes);
        let report = check(&bytes).unwrap();
        assert_eq!(report.rows(), 5);
        assert_eq!(report.columns().len(), 2);
        for column in report.columns() {
            assert_eq!(column.status(), LongValueCheckStatus::Checked);
            assert_eq!(column.values().nulls(), 1);
            assert_eq!(column.values().inline(), 2);
            assert_eq!(column.values().single(), 1);
            assert_eq!(column.values().chained(), 1);
            assert_eq!(column.distinct_chunks(), 3);
            assert_eq!(column.shared_references(), 0);
        }
        assert_eq!(report.columns()[0].owned_pages(), 4);
        assert_eq!(report.columns()[0].owned_rows(), 4);
        assert_eq!(report.columns()[0].orphan_rows(), 1);
        assert_eq!(report.columns()[1].orphan_rows(), 0);
        assert_eq!(report.columns()[0].maps().unwrap().owned(), (11 << 8) | 1);
        assert_eq!(bytes, before);
    }
}

#[test]
fn absent_unknown_historical_and_unallocated_map_profiles_preserve_read_boundaries() {
    let mut absent = model::database(6);
    support::put16(&mut absent, 10 * 4096 + model::TAIL, 65535);
    readable(&absent);
    assert!(
        check(&absent)
            .unwrap()
            .columns()
            .iter()
            .all(|column| column.status() == LongValueCheckStatus::Absent)
    );

    let mut unknown = model::database(6);
    unknown[10 * 4096 + model::TAIL..10 * 4096 + model::TAIL + 22].fill(0);
    readable(&unknown);
    let report = check(&unknown).unwrap();
    assert_eq!(
        report.columns()[0].status(),
        LongValueCheckStatus::UnknownDeclaration
    );
    assert_eq!(report.columns()[2].column_number(), 0);

    let mut historical = model::database(6);
    let start = 10 * 4096 + model::TAIL + 20;
    support::put16(&mut historical, start, 7);
    support::put32(&mut historical, start + 2, (11 << 8) | 1);
    support::put32(&mut historical, start + 6, (11 << 8) | 2);
    support::put16(&mut historical, start + 10, 65535);
    readable(&historical);
    let report = check(&historical).unwrap();
    assert_eq!(report.columns()[0].status(), LongValueCheckStatus::Checked);
    assert_eq!(report.columns()[2].column_number(), 7);
    assert_eq!(
        report.columns()[2].status(),
        LongValueCheckStatus::HistoricalDeclaration
    );

    let mut indirect = model::database(6);
    indirect.resize(22 * 4096, 0);
    model::maps(&mut indirect, vec![1, 21, 0, 0, 0]);
    indirect[21 * 4096..21 * 4096 + 2].copy_from_slice(&[5, 1]);
    for number in [14, 15, 16, 20] {
        indirect[21 * 4096 + 4 + number / 8] |= 1 << (number % 8);
    }
    readable(&indirect);
    // The original fixture marks bitmap page21 globally FREE. The newly
    // qualified mixed-map check must now reject that allocation contradiction.
    let start = u16::from_le_bytes(indirect[4096 + 14..4096 + 16].try_into().unwrap());
    assert_ne!(
        indirect[4096 + usize::from(start) + 5 + 21 / 8] & (1 << (21 % 8)),
        0
    );
    assert_eq!(check(&indirect).err(), Some(InspectError::Malformed));
}
