//! Original synthetic layouts test composition; these are not Access SQL goldens.
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/attachments/fixture.rs"]
mod attachment_fixture;
use tablecraft_testkit::access::mutation_fixture as base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "../../../engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod links_fixture;
use tablecraft_testkit::access as support;

use std::{
    cell::{Cell, RefCell},
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_ace::NativeReadSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare, source::TextPolicy};
use tablecraft_sql::{ParseLimits, parse_query};

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
fn order(bytes: &mut [u8], page: usize, index: usize, raw: [u8; 4]) {
    let at = page * PAGE_BYTES + FIELD_START + index * FIELD_BYTES + SORT_OFFSET;
    bytes[at..at + 4].copy_from_slice(&raw);
}
fn default_order(bytes: &mut [u8]) {
    // Independent masked header control: default General 2010, deliberately
    // different from the template/field in the legacy collection controls.
    for (i, mask) in [1, 27, 152, 253].into_iter().enumerate() {
        bytes[110 + i] = [9, 4, 0, 1][i] ^ mask;
    }
}
fn values(raw: [u8; 4]) -> Vec<u8> {
    let mut bytes = links_fixture::populated();
    default_order(&mut bytes);
    for (page, index, raw) in [(TEMPLATE_PAGE, 0, [0, 0, 0, 0]), (FLAT_PAGE, 1, raw)] {
        let at = page * PAGE_BYTES + FIELD_START + index * FIELD_BYTES;
        bytes[at] = 10;
        bytes[at + 15] = 2;
        support::put16(&mut bytes, at + 7, 0);
        support::put16(&mut bytes, at + 23, 16);
        support::put16(&mut bytes, page * PAGE_BYTES + 43, 1);
        order(&mut bytes, page, index, raw);
    }
    support::put16(
        &mut bytes,
        FLAT_PAGE * PAGE_BYTES + FIELD_START + 2 * FIELD_BYTES + 21,
        4,
    );
    let rows = [(10_i32, "a"), (10, "A"), (20, "\u{80}")]
        .into_iter()
        .enumerate()
        .map(|(i, (owner, text))| {
            let mut row = 3_u16.to_le_bytes().to_vec();
            row.extend(owner.to_le_bytes());
            row.extend((i as i32 + 1).to_le_bytes());
            let start = row.len() as u16;
            row.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
            row.extend((row.len() as u16).to_le_bytes());
            row.extend(start.to_le_bytes());
            row.extend(1_u16.to_le_bytes());
            row.push(7);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[CHILD_PAGE * PAGE_BYTES..(CHILD_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(FLAT_PAGE as u32, &rows));
    bytes
}
fn attachments() -> Vec<u8> {
    let mut bytes =
        attachment_fixture::with_payloads(attachment_fixture::PAYLOAD, attachment_fixture::PAYLOAD);
    default_order(&mut bytes);
    for (index, raw) in [(2, [0, 0, 0, 0]), (3, [9, 4, 0, 0]), (4, [9, 4, 0, 1])] {
        order(&mut bytes, FLAT_PAGE, index, raw);
        order(&mut bytes, TEMPLATE_PAGE, index - 2, [31, 4, 2, 7]);
    }
    bytes
}
fn execute(
    source: &dyn Source,
    sql: &str,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], options)?
        .rows()
        .to_vec())
}
#[test]
fn stored_value_policy_routes_projection_membership_like_group_and_pivot_across_profiles() {
    for code in [2, 5, 6] {
        for version in [0, 1] {
            let mut bytes = values([9, 4, 0, version]);
            bytes[20] = code;
            let pool = RefCell::new(budget());
            let (source, resolver) =
                NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
                    false
                })
                .unwrap()
                .with_multivalues()
                .with_database_collation_resolver(&pool)
                .unwrap();
            let id = source.multivalue_text_collations("Items").unwrap()[0].unwrap();
            assert_eq!(id.host_key(), u32::from_le_bytes([9, 4, 0, version]) as u64);
            let opts = || ExecutionOptions {
                collation: Some(resolver.default_collation()),
                collation_resolver: Some(&resolver),
                like_mode: Some(tablecraft_expressions::predicates::LikeMode::Ansi89),
                ..Default::default()
            };
            for sql in [
                "SELECT Id FROM Items WHERE Score.Value='A'",
                "SELECT A.Id FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id WHERE A.Score.Value='A'",
            ] {
                assert_eq!(
                    execute(&source, sql, opts()).unwrap(),
                    [vec![Value::Int32(1)]]
                );
            }
            assert_eq!(
                execute(
                    &source,
                    "SELECT Count(*) FROM Items WHERE Score.Value LIKE 'a'",
                    opts()
                )
                .unwrap(),
                [vec![Value::Int64(1)]]
            );
            assert_eq!(
                execute(
                    &source,
                    "SELECT Score.Value,Count(*) FROM Items GROUP BY Score.Value HAVING Count(*)>1",
                    opts()
                )
                .unwrap(),
                [vec![Value::Text("a".into()), Value::Int64(2)]]
            );
            assert_eq!(execute(&source,"TRANSFORM Count(*) SELECT 1 AS R FROM Items GROUP BY 1 PIVOT Score.Value IN ('A')",opts()).unwrap(),[vec![Value::Int64(1),Value::Int64(2)]]);
            // U+0080 distinguishes legacy and 2010; a global/database-default fallback
            // would return the same answer for both stored field versions.
            assert_eq!(
                execute(&source, "SELECT Id FROM Items WHERE Score.Value<'́'", opts())
                    .unwrap()
                    .iter()
                    .any(|row| row == &[Value::Int32(2)]),
                version == 0
            );
        }
    }
}
#[test]
fn attachment_metadata_keeps_each_order_and_decoded_binary_has_no_text_policy() {
    let pool = RefCell::new(budget());
    let (source, resolver) = NativeSource::new(
        Cursor::new(attachments()),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_decoded_attachments()
    .with_database_collation_resolver(&pool)
    .unwrap();
    let members = source.collection_members("Items").unwrap();
    let policies = source.collection_member_text_collations("Items").unwrap();
    assert_eq!(members.len(), policies.len());
    for (member, id) in members.iter().zip(policies) {
        let expected = match member.column().name() {
            "FileURL" => Some(0),
            "FileName" => Some(u32::from_le_bytes([9, 4, 0, 0]) as u64),
            "FileType" => Some(u32::from_le_bytes([9, 4, 0, 1]) as u64),
            _ => None,
        };
        assert_eq!(id.map(|id| id.host_key()), expected);
    }
    let opts = || ExecutionOptions {
        collation: Some(resolver.default_collation()),
        collation_resolver: Some(&resolver),
        ..Default::default()
    };
    assert_eq!(
        execute(
            &source,
            "SELECT Id FROM Items WHERE Score.FileType='FIELD/TYPE'",
            opts()
        )
        .unwrap(),
        [vec![Value::Int32(1)]]
    );
    assert_eq!(
        execute(
            &source,
            "SELECT Score.FileName=Score.FileType FROM Items",
            opts()
        )
        .err(),
        Some(QueryError::CollationConflict)
    );
    assert!(execute(&source, "SELECT Score.FileURL FROM Items", opts()).is_ok());
    assert_eq!(
        execute(&source, "SELECT Score.FileURL='x' FROM Items", opts()).err(),
        Some(tablecraft_expressions::EvalError::CollationRequired.into())
    );
    assert!(execute(&source, "SELECT Score.FileData FROM Items", opts()).is_ok());
}

struct Shared {
    bytes: Rc<RefCell<Vec<u8>>>,
    position: u64,
    forbid: Rc<Cell<bool>>,
}
impl NativeReadSource for Shared {}
impl Read for Shared {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let bytes = self.bytes.borrow();
        let start = self.position as usize;
        let count = buf.len().min(bytes.len().saturating_sub(start));
        if count != 0 && self.forbid.get() {
            for page in [12, CHILD_PAGE] {
                assert!(
                    start + count <= page * PAGE_BYTES || start >= (page + 1) * PAGE_BYTES,
                    "binding read user rows"
                );
            }
        }
        if count != 0 {
            buf[..count].copy_from_slice(&bytes[start..start + count]);
        }
        self.position += count as u64;
        Ok(count)
    }
}
impl Seek for Shared {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.position = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::End(n) => (self.bytes.borrow().len() as u64)
                .checked_add_signed(n)
                .unwrap(),
            SeekFrom::Current(n) => self.position.checked_add_signed(n).unwrap(),
        };
        Ok(self.position)
    }
}
#[test]
fn collection_binding_is_metadata_only_and_snapshot_sort_drift_fails() {
    for attachment in [false, true] {
        let bytes = Rc::new(RefCell::new(if attachment {
            attachments()
        } else {
            values([9, 4, 0, 0])
        }));
        let forbid = Rc::new(Cell::new(true));
        let pool = RefCell::new(budget());
        let (source, resolver) = NativeSource::new(
            Shared {
                bytes: bytes.clone(),
                position: 0,
                forbid: forbid.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_collections()
        .with_database_collation_resolver(&pool)
        .unwrap();
        let parsed = parse_query(
            if attachment {
                "SELECT Score.FileName FROM Items"
            } else {
                "SELECT Score.Value FROM Items"
            },
            ParseLimits::default(),
        )
        .unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert!(matches!(
            plan.output_text_policies().unwrap()[0],
            TextPolicy::Column(_)
        ));
        order(
            &mut bytes.borrow_mut(),
            FLAT_PAGE,
            if attachment { 3 } else { 1 },
            [9, 4, 0, 1],
        );
        forbid.set(false);
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                }
            )
            .err(),
            Some(QueryError::SchemaChanged)
        );
    }
}
#[test]
fn unknown_element_policy_is_inert_until_comparison_requires_it() {
    let pool = RefCell::new(budget());
    let (source, resolver) = NativeSource::new(
        Cursor::new(values([31, 4, 2, 7])),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_multivalues()
    .with_database_collation_resolver(&pool)
    .unwrap();
    let opts = || ExecutionOptions {
        collation: Some(resolver.default_collation()),
        collation_resolver: Some(&resolver),
        ..Default::default()
    };
    assert!(execute(&source, "SELECT Score.Value FROM Items", opts()).is_ok());
    assert_eq!(
        execute(&source, "SELECT Score.Value='a' FROM Items", opts()).err(),
        Some(tablecraft_expressions::EvalError::CollationRequired.into())
    );
}
