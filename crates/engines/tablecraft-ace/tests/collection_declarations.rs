#[path = "attachments/fixture.rs"]
mod attachment_fixture;
#[path = "mutation/fixture.rs"]
#[allow(dead_code)]
mod base;
#[path = "complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "complex_links/fixture.rs"]
mod links_fixture;
mod support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_ace::{
    InspectError, NativeReadSource, NativeSortOrder, inspect_attachment_declarations,
    inspect_attachment_members, inspect_multivalue_columns, inspect_multivalue_declarations,
    read_attachment_column, read_multivalue_column,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const PAGE_BYTES: usize = 4096;
const TEMPLATE_PAGE: usize = 17;
const FLAT_PAGE: usize = 20;
const CHILD_PAGE: usize = 22;
const FIELD_START: usize = 63;
const FIELD_BYTES: usize = 25;
const SORT_OFFSET: usize = 11;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000)
}
fn text_fixture(template: [u8; 4], flat: [u8; 4]) -> Vec<u8> {
    let mut bytes = links_fixture::populated();
    for (page, index, order) in [(TEMPLATE_PAGE, 0, template), (FLAT_PAGE, 1, flat)] {
        let at = page * PAGE_BYTES + FIELD_START + index * FIELD_BYTES;
        bytes[at] = 10;
        bytes[at + 15] = 2;
        support::put16(&mut bytes, at + 7, 0);
        support::put16(&mut bytes, at + 23, 16);
        bytes[at + SORT_OFFSET..at + SORT_OFFSET + 4].copy_from_slice(&order);
        support::put16(&mut bytes, page * PAGE_BYTES + 43, 1);
    }
    support::put16(
        &mut bytes,
        FLAT_PAGE * PAGE_BYTES + FIELD_START + 2 * FIELD_BYTES + 21,
        4,
    );
    let records = [(10_i32, "a"), (10, "A"), (20, "\u{80}")]
        .into_iter()
        .enumerate()
        .map(|(i, (owner, value))| {
            let mut row = 3_u16.to_le_bytes().to_vec();
            row.extend(owner.to_le_bytes());
            row.extend((i as i32 + 1).to_le_bytes());
            let start = row.len() as u16;
            row.extend(value.encode_utf16().flat_map(u16::to_le_bytes));
            row.extend((row.len() as u16).to_le_bytes());
            row.extend(start.to_le_bytes());
            row.extend(1_u16.to_le_bytes());
            row.push(7);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[CHILD_PAGE * PAGE_BYTES..(CHILD_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(FLAT_PAGE as u32, &records));
    bytes
}
fn attachment_orders() -> (Vec<u8>, [[u8; 4]; 3]) {
    let mut bytes = attachment_fixture::database();
    let orders = [[9, 4, 0, 0], [9, 4, 0, 1], [31, 4, 2, 7]];
    for (i, order) in orders.iter().enumerate() {
        let at = FLAT_PAGE * PAGE_BYTES + FIELD_START + (i + 2) * FIELD_BYTES + SORT_OFFSET;
        bytes[at..at + 4].copy_from_slice(order);
        let template = TEMPLATE_PAGE * PAGE_BYTES + FIELD_START + i * FIELD_BYTES + SORT_OFFSET;
        bytes[template..template + 4].copy_from_slice(&[0, 0, 0, 0]);
    }
    (bytes, orders)
}
struct Guard {
    cursor: Cursor<Vec<u8>>,
    attempts: Rc<Cell<usize>>,
}
impl NativeReadSource for Guard {}
impl Read for Guard {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let start = self.cursor.position();
        let end = start.saturating_add(buffer.len() as u64);
        if [12, CHILD_PAGE].into_iter().any(|page| {
            start < (page + 1) as u64 * PAGE_BYTES as u64 && end > page as u64 * PAGE_BYTES as u64
        }) {
            self.attempts.set(self.attempts.get() + 1);
            return Err(std::io::Error::other("user record pages forbidden"));
        }
        self.cursor.read(buffer)
    }
}
impl Seek for Guard {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(position)
    }
}
#[test]
fn multivalue_declarations_and_snapshots_keep_flat_unknown_orders_without_template_fallback() {
    for code in [2, 5, 6] {
        for raw in [[9, 4, 0, 0], [9, 4, 0, 1], [0, 0, 0, 0], [31, 4, 2, 7]] {
            let mut bytes = text_fixture([9, 4, 1, 9], raw);
            bytes[20] = code;
            let rich = inspect_multivalue_declarations(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rich.len(), 1);
            assert_eq!(rich[0].column().name(), "Score");
            assert_eq!(rich[0].column().data_type(), ScalarType::Text);
            assert_eq!(
                rich[0].text_sort_order(),
                Some(NativeSortOrder::from_native_bytes(raw))
            );
            let old = inspect_multivalue_columns(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(
                rich.into_iter()
                    .map(|d| d.into_column())
                    .collect::<Vec<_>>()
                    == old
            );
            let snapshot = read_multivalue_column(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Score",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                snapshot.value_text_sort_order(),
                Some(NativeSortOrder::from_native_bytes(raw))
            );
            assert_eq!(snapshot.value(0, 0), Some(&Value::Text("a".into())));
            assert_eq!(snapshot.value(0, 1), Some(&Value::Text("A".into())));
        }
    }
    let bytes = links_fixture::populated();
    let rich = inspect_multivalue_declarations(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rich[0].text_sort_order(), None);
    assert_eq!(
        read_multivalue_column(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .value_text_sort_order(),
        None
    );
}
#[test]
fn attachment_declarations_preserve_member_orders_and_keep_encoded_file_data_absent() {
    for code in [2, 5, 6] {
        let (mut bytes, orders) = attachment_orders();
        bytes[20] = code;
        let rich = inspect_attachment_declarations(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(rich.len(), 5);
        for (index, member) in rich.iter().enumerate() {
            assert_eq!(member.member().collection().as_str(), "Score");
            assert_eq!(
                member.text_sort_order(),
                orders
                    .get(index)
                    .copied()
                    .map(NativeSortOrder::from_native_bytes)
            );
        }
        let old = inspect_attachment_members(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            rich.into_iter()
                .map(|d| d.into_member())
                .collect::<Vec<_>>()
                == old
        );
        let snapshot = read_attachment_column(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for (name, order) in ["FileURL", "filename", "FileType"].into_iter().zip(orders) {
            assert_eq!(
                snapshot.metadata_text_sort_order(name),
                Some(NativeSortOrder::from_native_bytes(order))
            );
        }
        for name in ["FileData", "FileTimeStamp", "FileFlags", "private-unknown"] {
            assert_eq!(snapshot.metadata_text_sort_order(name), None);
        }
    }
}
#[test]
fn declaration_discovery_never_reads_parent_or_child_records() {
    for attachment in [false, true] {
        let bytes = if attachment {
            attachment_orders().0
        } else {
            text_fixture([9, 4, 0, 0], [9, 4, 0, 1])
        };
        let attempts = Rc::new(Cell::new(0));
        let mut reader = Guard {
            cursor: Cursor::new(bytes),
            attempts: attempts.clone(),
        };
        if attachment {
            assert_eq!(
                inspect_attachment_declarations(
                    &mut reader,
                    CatalogScope::new(1),
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .len(),
                5
            );
        } else {
            assert_eq!(
                inspect_multivalue_declarations(
                    &mut reader,
                    CatalogScope::new(1),
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .len(),
                1
            );
        }
        assert_eq!(attempts.get(), 0);
        if attachment {
            assert!(
                read_attachment_column(
                    &mut reader,
                    CatalogScope::new(1),
                    "Items",
                    "Score",
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
        } else {
            assert!(
                read_multivalue_column(
                    &mut reader,
                    CatalogScope::new(1),
                    "Items",
                    "Score",
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
        }
        assert!(attempts.get() > 0);
    }
}
#[test]
fn declaration_budget_and_late_cancellation_errors_publish_no_result() {
    for attachment in [false, true] {
        let bytes = if attachment {
            attachment_orders().0
        } else {
            text_fixture([9, 4, 0, 0], [9, 4, 0, 1])
        };
        let visits = Cell::new(0usize);
        let callback = || {
            visits.set(visits.get() + 1);
            false
        };
        if attachment {
            inspect_attachment_declarations(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &callback,
            )
            .unwrap();
        } else {
            inspect_multivalue_declarations(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &callback,
            )
            .unwrap();
        }
        let total = visits.get();
        assert!(total > 1);
        for point in [1, total / 2, total] {
            visits.set(0);
            let cancelled = || {
                let n = visits.get() + 1;
                visits.set(n);
                n == point
            };
            let result = if attachment {
                inspect_attachment_declarations(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    &mut budget(),
                    &cancelled,
                )
                .map(|v| v.len())
            } else {
                inspect_multivalue_declarations(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    &mut budget(),
                    &cancelled,
                )
                .map(|v| v.len())
            };
            assert_eq!(result, Err(InspectError::Cancelled));
        }
        let result = if attachment {
            inspect_attachment_declarations(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut ResourceBudget::new(0, 0, 0),
                &|| false,
            )
            .map(|v| v.len())
        } else {
            inspect_multivalue_declarations(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut ResourceBudget::new(0, 0, 0),
                &|| false,
            )
            .map(|v| v.len())
        };
        assert!(matches!(result, Err(InspectError::Limit(_))));
    }
}
