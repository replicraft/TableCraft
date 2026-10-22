use super::*;
use std::cell::Cell;
use std::io::{Read, Seek, SeekFrom};
use tablecraft_ace::{
    GuidIndexSelection as Selection, IndexProjection, IndexProjectionSelection,
    read_table_guid_index, read_table_index_projection,
};
use tablecraft_core::values::Guid;

fn selected(
    bytes: &[u8],
    selection: Selection,
) -> Result<tablecraft_ace::TableSnapshot, InspectError> {
    read_table_guid_index(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        selection,
        &mut budget(),
        &|| false,
    )
}

fn encoded(value: Option<[u8; 16]>, slot: usize, descending: bool) -> Vec<u8> {
    let mut key = vec![match (value, descending) {
        (None, false) => 0,
        (None, true) => 0xff,
        (Some(_), false) => 0x7f,
        (Some(_), true) => 0x80,
    }];
    if let Some(value) = value {
        key.extend(
            value[..8]
                .iter()
                .map(|byte| if descending { !byte } else { *byte }),
        );
        key.push(9);
        key.extend(
            value[8..]
                .iter()
                .map(|byte| if descending { !byte } else { *byte }),
        );
        key.push(if descending { 0xf7 } else { 8 });
    }
    key.extend(((12_u32 << 8) | slot as u32).to_be_bytes());
    key
}

fn page(leaf: bool, entries: &[Vec<u8>], previous: u32, next: u32, tail: u32) -> Vec<u8> {
    let mut page = vec![0; 4096];
    page[..2].copy_from_slice(&[if leaf { 4 } else { 3 }, 1]);
    for (at, value) in [(4, 10_u32), (12, previous), (16, next), (20, tail)] {
        page[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    let mut end = 0;
    for entry in entries {
        page[480 + end..480 + end + entry.len()].copy_from_slice(entry);
        end += entry.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    page[2..4].copy_from_slice(&((3616 - end) as u16).to_le_bytes());
    page
}

fn tree(descending: bool, split: usize) -> Vec<u8> {
    let values = [
        None,
        None,
        Some([0; 16]),
        Some([0x80; 16]),
        Some([0x80; 16]),
        Some([0xff; 16]),
    ];
    let mut bytes = fixture::database(6, descending, &values, 0);
    bytes.resize(17 * 4096, 0);
    let mut order = values.into_iter().enumerate().collect::<Vec<_>>();
    order.sort_by(|a, b| {
        let order = a.1.cmp(&b.1);
        (if descending { order.reverse() } else { order }).then(a.0.cmp(&b.0))
    });
    let entries = order
        .into_iter()
        .map(|(slot, value)| encoded(value, slot, descending))
        .collect::<Vec<_>>();
    let mut separator = entries[split - 1].clone();
    separator.extend(15_u32.to_be_bytes());
    bytes[14 * 4096..15 * 4096].copy_from_slice(&page(false, &[separator], 0, 0, 16));
    bytes[15 * 4096..16 * 4096].copy_from_slice(&page(true, &entries[..split], 0, 16, 0));
    bytes[16 * 4096..17 * 4096].copy_from_slice(&page(true, &entries[split..], 15, 0, 0));
    bytes[13 * 4096..14 * 4096].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0xc0, 1])],
    ));
    bytes
}

#[test]
fn guid_points_keep_duplicate_boundary_keys_nulls_and_canonical_bits() {
    for descending in [false, true] {
        for split in 1..6 {
            let bytes = tree(descending, split);
            assert_eq!(
                pairs(&selected(&bytes, Selection::All).unwrap()),
                pairs(&read(&bytes).unwrap())
            );
            for (selection, ids) in [
                (Selection::Null, vec![1, 2]),
                (Selection::Equal(Guid::from_bytes([0; 16])), vec![3]),
                (Selection::Equal(Guid::from_bytes([0x80; 16])), vec![4, 5]),
                (Selection::Equal(Guid::from_bytes([0xff; 16])), vec![6]),
                (Selection::Equal(Guid::from_bytes([1; 16])), vec![]),
            ] {
                let actual = pairs(&selected(&bytes, selection).unwrap())
                    .into_iter()
                    .map(|(_, id)| id)
                    .collect::<Vec<_>>();
                assert_eq!(actual, ids);
            }
        }
    }
}

struct Touches {
    source: Cursor<Vec<u8>>,
    pages: Vec<u64>,
}

impl tablecraft_ace::NativeReadSource for Touches {}
impl Read for Touches {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        self.source.read(output)
    }
}
impl Seek for Touches {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        if let SeekFrom::Start(offset) = position {
            self.pages.push(offset / 4096);
        }
        self.source.seek(position)
    }
}

#[test]
fn disjoint_pages_and_unmatched_records_are_not_read_but_selected_corruption_fails() {
    let mut bytes = tree(false, 4);
    bytes[15 * 4096] = 0;
    let mut source = Touches {
        source: Cursor::new(bytes.clone()),
        pages: vec![],
    };
    let result = read_table_guid_index(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Compound",
        Selection::Equal(Guid::from_bytes([0xff; 16])),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(pairs(&result), vec![(Some([0xff; 16]), 6)]);
    assert!(!source.pages.contains(&15));
    assert!(selected(&bytes, Selection::All).is_err());
    assert!(selected(&bytes, Selection::Equal(Guid::from_bytes([0x80; 16]))).is_err());
    let mut source = Touches {
        source: Cursor::new(tree(false, 4)),
        pages: vec![],
    };
    let result = read_table_guid_index(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Compound",
        Selection::Equal(Guid::from_bytes([1; 16])),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(result.rows().is_empty());
    assert!(!source.pages.contains(&12));
}

#[test]
fn guid_selections_reject_other_carriers_and_composites_even_for_all_or_null() {
    for columns in [vec![(1, false)], vec![(0, false), (1, false)]] {
        let bytes = numeric_fixture::database(6, &[4, 4], &columns, &[vec![Some(1), Some(2)]], 0);
        for selection in [
            Selection::All,
            Selection::Null,
            Selection::Equal(Guid::from_bytes([0; 16])),
        ] {
            assert_eq!(
                selected(&bytes, selection).err(),
                Some(InspectError::Unsupported)
            );
        }
    }
}

proptest! {
    #[test]
    fn guid_point_reads_match_an_independent_full_key_filter(
        values in prop::collection::vec(prop::option::of(any::<[u8;16]>()),0..40),
        target in any::<[u8;16]>(),
        descending in any::<bool>(),
    ) {
        let bytes = fixture::database(6,descending,&values,0);
        let expected = pairs(&read(&bytes).unwrap()).into_iter().filter(|(key,_)| *key==Some(target)).collect::<Vec<_>>();
        prop_assert_eq!(pairs(&selected(&bytes,Selection::Equal(Guid::from_bytes(target))).unwrap()),expected);
        for value in values.iter().flatten().take(3) {
            let expected = pairs(&read(&bytes).unwrap()).into_iter().filter(|(key,_)| *key==Some(*value)).collect::<Vec<_>>();
            prop_assert_eq!(pairs(&selected(&bytes,Selection::Equal(Guid::from_bytes(*value))).unwrap()),expected);
        }
    }
}

#[test]
fn selected_rows_retain_key_agreement_and_partial_counts_do_not_claim_full_audits() {
    let original = fixture::database(6, false, &[Some([0x80; 16])], 0);
    let mut changed = original.clone();
    let at = 12 * 4096 + 14;
    let row = u16::from_le_bytes(changed[at..at + 2].try_into().unwrap()) as usize & 0x1fff;
    changed[12 * 4096 + row + 6] ^= 1;
    assert_eq!(
        selected(&changed, Selection::Equal(Guid::from_bytes([0x80; 16]))).err(),
        Some(InspectError::Malformed)
    );
    let mut changed = original;
    changed[10 * 4096 + 16..10 * 4096 + 20].copy_from_slice(&2_u32.to_le_bytes());
    assert!(selected(&changed, Selection::All).is_err());
    assert_eq!(
        selected(&changed, Selection::Equal(Guid::from_bytes([0x80; 16])))
            .unwrap()
            .rows()
            .len(),
        1
    );
}

#[test]
fn guid_point_work_remains_bounded_and_every_observed_checkpoint_can_cancel() {
    let bytes = tree(false, 4);
    let calls = Cell::new(0);
    let stop = Cell::new(usize::MAX);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == stop.get()
    };
    let execute = || {
        read_table_guid_index(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Compound",
            Selection::Equal(Guid::from_bytes([0x80; 16])),
            &mut budget(),
            &cancelled,
        )
    };
    execute().unwrap();
    for checkpoint in 1..=calls.get() {
        calls.set(0);
        stop.set(checkpoint);
        assert_eq!(execute().err(), Some(InspectError::Cancelled));
    }
    for limits in [
        ResourceBudget::new(1, 16384, 1_000_000),
        ResourceBudget::new(32 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(32 * 1024 * 1024, 16384, 1),
    ] {
        let mut limits = limits;
        assert!(matches!(
            read_table_guid_index(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Compound",
                Selection::Equal(Guid::from_bytes([0x80; 16])),
                &mut limits,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
}

struct Faults {
    source: Cursor<Vec<u8>>,
    calls: usize,
    fail_at: usize,
}

impl Faults {
    fn step(&mut self) -> std::io::Result<()> {
        self.calls += 1;
        if self.calls == self.fail_at {
            return Err(std::io::Error::other("injected GUID seek failure"));
        }
        Ok(())
    }
}

impl tablecraft_ace::NativeReadSource for Faults {}
impl Read for Faults {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        self.step()?;
        self.source.read(output)
    }
}
impl Seek for Faults {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.step()?;
        self.source.seek(position)
    }
}

#[test]
fn every_selected_path_io_failure_refuses_a_snapshot() {
    let mut source = Faults {
        source: Cursor::new(tree(false, 4)),
        calls: 0,
        fail_at: usize::MAX,
    };
    let execute = |source: &mut Faults| {
        read_table_guid_index(
            source,
            CatalogScope::new(1),
            "Items",
            "Compound",
            Selection::Equal(Guid::from_bytes([0x80; 16])),
            &mut budget(),
            &|| false,
        )
    };
    execute(&mut source).unwrap();
    for fail_at in 1..=source.calls {
        let mut source = Faults {
            source: Cursor::new(tree(false, 4)),
            calls: 0,
            fail_at,
        };
        assert!(execute(&mut source).is_err(), "I/O callback {fail_at}");
    }
}

#[test]
fn projected_guid_selections_validate_hidden_keys_and_reject_other_key_types() {
    for descending in [false, true] {
        let bytes = fixture::database(6, descending, &[None, Some([0; 16]), Some([0x80; 16])], 0);
        for selection in [
            Selection::All,
            Selection::Null,
            Selection::Equal(Guid::from_bytes([0x80; 16])),
        ] {
            let snapshot = read_table_index_projection(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Compound",
                IndexProjection {
                    columns: &["Id"],
                    selection: IndexProjectionSelection::Guid(selection),
                },
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let expected = selected(&bytes, selection)
                .unwrap()
                .rows()
                .iter()
                .map(|row| vec![row[0].clone()])
                .collect::<Vec<_>>();
            assert_eq!(snapshot.rows(), expected);
        }
    }
    let mut bytes = fixture::database(6, false, &[Some([0x80; 16])], 0);
    let offset = 12 * 4096 + 14;
    let row = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize & 0x1fff;
    bytes[12 * 4096 + row + 6] ^= 1;
    assert_eq!(
        read_table_index_projection(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Compound",
            IndexProjection {
                columns: &["Id"],
                selection: IndexProjectionSelection::Guid(Selection::Equal(Guid::from_bytes(
                    [0x80; 16]
                )))
            },
            &mut budget(),
            &|| false,
        )
        .err(),
        Some(InspectError::Malformed)
    );
    let bytes =
        numeric_fixture::database(6, &[4, 0x13], &[(1, false)], &[vec![Some(1), Some(2)]], 0);
    for selection in [Selection::All, Selection::Null] {
        assert!(
            read_table_index_projection(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Compound",
                IndexProjection {
                    columns: &["Id"],
                    selection: IndexProjectionSelection::Guid(selection)
                },
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
    }
}
