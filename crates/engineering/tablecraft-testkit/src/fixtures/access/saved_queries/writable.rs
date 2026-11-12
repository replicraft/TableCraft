//! Canonical native17 ACE16 source with complete inert saved-query storage.
use super::{EMPTY_ID, Fragment, QUERY_ID, fragment_records};
use crate::fixtures::access::{
    canonical_creation::{self as canonical, Field},
    canonical_native17_creation as native17,
    canonical_retained_add_table::extend_global_map,
    packed_record::append_record,
    put16, put32,
};

const PAGE: usize = 4096;
const TABLES: u32 = 0x0f00_0001;
const USER_DEFINITION: usize = 7;
const USER_DATA: usize = 8;
const QUERY_DEFINITION: usize = 9;
const QUERY_DATA: usize = 10;
const MAP_PAGE: usize = 3;
const CATALOG_DEFINITION: usize = 2;
const CATALOG_DATA: usize = 4;
const ORDER: [u8; 4] = [0, 0, 0, 1];
const QUERY_FIELDS: [Field<'static>; 8] = [
    Field::fixed("ObjectId", 4, 4, 0, 0),
    Field::fixed("Attribute", 2, 1, 4, 0),
    Field::variable("Order", 9, 4, 0),
    Field::variable("Name1", 10, 510, 1),
    Field::variable("Name2", 10, 510, 2),
    Field::variable("Expression", 12, 0, 3),
    Field::fixed("Flag", 3, 2, 5, 4),
    Field::fixed("LvExtra", 4, 4, 7, 4),
];

/// Three ordinary Items rows plus Saved/Empty SELECTs with independent parameters.
/// The expected image applies Score+7 only to Id>=2 and protects every definition.
pub struct Source {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
}

/// Both images are authored independently from the same literal raw schema facts.
/// This is the native17 canonical source grammar, not a relabeled Jet4 fixture.
pub fn source() -> Source {
    Source {
        before: database(&[(1, 10), (2, 20), (3, 30)]),
        expected: database(&[(1, 10), (2, 27), (3, 37)]),
    }
}

fn fragment(object: i32, attribute: u8) -> Fragment<'static> {
    Fragment {
        object: Some(object),
        attribute: Some(attribute),
        order: Some(&ORDER),
        ..Default::default()
    }
}

fn fragments() -> Vec<Fragment<'static>> {
    let mut fragments = Vec::new();
    for (object, parameter, predicate) in [
        (QUERY_ID, "First minimum", "Id >= [First minimum]"),
        (EMPTY_ID, "Second maximum", "Id <= [Second maximum]"),
    ] {
        fragments.extend([
            fragment(object, 0),
            Fragment {
                flag: Some(1),
                ..fragment(object, 1)
            },
            Fragment {
                name1: Some(parameter),
                flag: Some(4),
                ..fragment(object, 2)
            },
            Fragment {
                flag: Some(0),
                ..fragment(object, 3)
            },
            Fragment {
                name1: Some("Items"),
                ..fragment(object, 5)
            },
            Fragment {
                expression: Some("Id"),
                ..fragment(object, 6)
            },
            Fragment {
                expression: Some(predicate),
                ..fragment(object, 8)
            },
            fragment(object, 255),
        ]);
    }

    fragments
}

fn database(values: &[(i32, i32)]) -> Vec<u8> {
    let mut bytes = native17::source(canonical::source(canonical::Case::One).bytes);
    assert_eq!(bytes.len(), QUERY_DEFINITION * PAGE);
    let records = values
        .iter()
        .map(|(id, score)| {
            let mut record = 2_u16.to_le_bytes().to_vec();
            record.extend(id.to_le_bytes());
            record.extend(score.to_le_bytes());
            record.push(3);
            record
        })
        .collect();
    bytes[USER_DATA * PAGE..(USER_DATA + 1) * PAGE]
        .copy_from_slice(&native17::packed(USER_DEFINITION as u32, records));
    put32(&mut bytes, USER_DEFINITION * PAGE + 16, values.len() as u32);

    let maps = usize::from(u16::from_le_bytes(
        bytes[MAP_PAGE * PAGE + 12..MAP_PAGE * PAGE + 14]
            .try_into()
            .unwrap(),
    ));
    assert!(maps + 4 <= 256);
    for member in [Some(QUERY_DATA), Some(QUERY_DATA), None, None] {
        let mut map = [0; 13];
        if let Some(member) = member {
            map[5 + member / 8] = 1 << (member % 8);
        }
        append_record(&mut bytes[MAP_PAGE * PAGE..(MAP_PAGE + 1) * PAGE], &map);
    }

    let fragments = fragments();
    let mut definition = canonical::definition(&QUERY_FIELDS, fragments.len(), maps as u8);
    put32(
        &mut definition,
        59,
        (MAP_PAGE as u32) << 8 | (maps + 1) as u32,
    );
    let length = u32::from_le_bytes(definition[8..12].try_into().unwrap()) as usize;
    let tail = length - 2;
    assert_eq!(&definition[tail..length], &[255, 255]);

    // Expression's inline Memo still has represented, distinct empty LVAL maps.
    put16(&mut definition, tail, 5);
    put32(
        &mut definition,
        tail + 2,
        (MAP_PAGE as u32) << 8 | (maps + 2) as u32,
    );
    put32(
        &mut definition,
        tail + 6,
        (MAP_PAGE as u32) << 8 | (maps + 3) as u32,
    );
    put16(&mut definition, tail + 10, 0xffff);
    put32(&mut definition, 8, (length + 10) as u32);
    put16(&mut definition, 2, (PAGE - length - 18) as u16);
    bytes.extend(definition);
    bytes.extend(native17::packed(
        QUERY_DEFINITION as u32,
        fragment_records(&fragments)
            .into_iter()
            .map(|(_, row)| row)
            .collect(),
    ));

    for (name, id, kind, flags) in [
        ("MSysQueries", QUERY_DEFINITION as u32, 1, 0x8000_0002),
        ("Saved", QUERY_ID as u32, 5, 0),
        ("Empty", EMPTY_ID as u32, 5, 0),
    ] {
        let row = native17::catalog_row(&canonical::catalog_record(id, TABLES, name, kind, flags));
        append_record(
            &mut bytes[CATALOG_DATA * PAGE..(CATALOG_DATA + 1) * PAGE],
            &row,
        );
    }

    let count_at = CATALOG_DEFINITION * PAGE + 16;
    let count = u32::from_le_bytes(bytes[count_at..count_at + 4].try_into().unwrap());
    put32(&mut bytes, count_at, count + 3);
    extend_global_map(&mut bytes, QUERY_DEFINITION);

    bytes
}

#[cfg(test)]
mod tests {
    use super::source;

    #[test]
    fn authored_update_changes_only_two_score_bytes_and_preserves_saved_storage() {
        let source = source();
        assert_eq!(source.before.len(), 11 * 4096);
        assert_eq!(source.before.len(), source.expected.len());

        let changed = source
            .before
            .iter()
            .zip(&source.expected)
            .enumerate()
            .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
            .collect::<Vec<_>>();

        // Three eleven-byte records end at 4096; each Score begins six bytes
        // after its record's own start. Catalog/query/LVAL metadata is untouched.
        assert_eq!(changed, [8 * 4096 + 4069, 8 * 4096 + 4080]);
        assert_eq!(source.before[changed[0]], 30);
        assert_eq!(source.expected[changed[0]], 37);
        assert_eq!(source.before[changed[1]], 20);
        assert_eq!(source.expected[changed[1]], 27);
    }
}
