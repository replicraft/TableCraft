//! Actual immutable Sentinel bag, with independent literal record controls.
use super::validate;
use crate::{InspectError, NativeImage, TableProperties, inspect_table_properties};
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Cursor, path::Path};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(131);
const PUBLISH: [u8; 57] = [
    77, 82, 50, 0, 32, 0, 0, 0, 128, 0, 24, 0, 80, 0, 117, 0, 98, 0, 108, 0, 105, 0, 115, 0, 104,
    0, 84, 0, 111, 0, 87, 0, 101, 0, 98, 0, 21, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 9, 0, 0, 2, 0, 0,
    1, 0, 0,
];

pub(crate) struct Source {
    pub bytes: Vec<u8>,
    pub property_offset: usize,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

/// Shared by these framing controls and the actual admission-helper controls.
/// Files are original public-builder specimens, not newly fabricated bags.
pub(crate) fn originals() -> Vec<Source> {
    let directory = std::env::var_os("TABLECRAFT_GENUINE_PROPERTY_SENTINEL_SOURCES_DIR").unwrap();
    let directory = Path::new(&directory);
    assert!(directory.is_absolute());
    [
        (
            "h5.accdb",
            458752,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            450474,
        ),
        (
            "h6.accdb",
            528384,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            470685,
        ),
    ]
    .into_iter()
    .map(|(filename, length, digest, property_offset)| {
        let bytes = std::fs::read(directory.join(filename)).unwrap();
        assert_eq!(bytes.len(), length);
        let actual = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, digest);
        assert_eq!(&bytes[property_offset..property_offset + 57], &PUBLISH);
        Source {
            bytes,
            property_offset,
        }
    })
    .collect()
}

fn inspect(source: &Source) -> TableProperties {
    let image = NativeImage::capture(
        &mut Cursor::new(&source.bytes),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let properties = inspect_table_properties(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(properties.raw_bytes(), PUBLISH);
    assert_eq!(properties.groups().len(), 1);
    let property = &properties.groups()[0].properties()[0];
    assert_eq!(property.name(), "PublishToWeb");
    assert_eq!(property.native_type(), 2);
    assert_eq!(property.flags(), 0);
    assert_eq!(property.byte().unwrap(), 0);
    properties
}

fn decoded(original: &TableProperties, raw: Vec<u8>) -> TableProperties {
    let groups = super::super::decode::decode(&raw, &mut budget(), &|| false).unwrap();
    // Metadata-only test view of the actual permissive reader. No certificate
    // or Source can be built from this internal decoded property value.
    TableProperties {
        name: original.name().to_owned(),
        object: original.object_id(),
        raw,
        groups,
    }
}

#[test]
#[ignore = "requires two literal-hash-pinned genuine originals"]
fn genuine_passive_bag_is_complete_and_permissive_record_tails_do_not_become_authority() {
    for source in originals() {
        let original = inspect(&source);
        validate(&original, &mut budget(), &|| false).unwrap();

        let mut record_tail = PUBLISH.to_vec();
        record_tail.extend_from_slice(&[0xa5]);
        record_tail[36..40].copy_from_slice(&22_u32.to_le_bytes());
        record_tail[48..50].copy_from_slice(&10_u16.to_le_bytes());
        let permissive = decoded(&original, record_tail);
        let property = &permissive.groups()[0].properties()[0];
        assert_eq!(property.raw_value(), &[0]);
        assert_eq!(property.byte().unwrap(), 0);
        assert!(validate(&permissive, &mut budget(), &|| false).is_err());

        for raw in [Vec::new(), b"MR2\0".to_vec(), PUBLISH[..36].to_vec()] {
            let incomplete = decoded(&original, raw);
            assert!(validate(&incomplete, &mut budget(), &|| false).is_err());
        }
        let mut empty_group = PUBLISH[..48].to_vec();
        empty_group[36..40].copy_from_slice(&12_u32.to_le_bytes());
        assert!(validate(&decoded(&original, empty_group), &mut budget(), &|| false).is_err());

        let mut unused_name = PUBLISH.to_vec();
        unused_name.splice(
            36..36,
            [12, 0, 85, 0, 110, 0, 117, 0, 115, 0, 101, 0, 100, 0],
        );
        unused_name[4..8].copy_from_slice(&46_u32.to_le_bytes());
        assert!(validate(&decoded(&original, unused_name), &mut budget(), &|| false).is_err());

        let mut unknown_group = PUBLISH.to_vec();
        // Non-table groups require a nonempty name even during inert decode.
        // Give this unknown group a valid name so strict framing is the guard.
        unknown_group.splice(48..48, [65, 0]);
        unknown_group[36..40].copy_from_slice(&23_u32.to_le_bytes());
        unknown_group[40..42].copy_from_slice(&3_u16.to_le_bytes());
        unknown_group[42..46].copy_from_slice(&8_u32.to_le_bytes());
        unknown_group[46..48].copy_from_slice(&2_u16.to_le_bytes());
        assert!(validate(&decoded(&original, unknown_group), &mut budget(), &|| false).is_err());
        let mut trailing = PUBLISH.to_vec();
        trailing.push(0xff);
        assert!(super::super::decode::decode(&trailing, &mut budget(), &|| false).is_err());
        assert!(super::super::decode::decode(b"<a/>", &mut budget(), &|| false).is_err());
        assert_eq!(
            &source.bytes[source.property_offset..source.property_offset + 57],
            &PUBLISH
        );
    }
}

#[test]
#[ignore = "requires two literal-hash-pinned genuine originals"]
fn strict_property_framing_pays_metadata_pools_and_checks_every_cancel_boundary() {
    for source in originals() {
        let properties = inspect(&source);
        let mut measured = budget();
        let initial = [Resource::Bytes, Resource::Steps].map(|pool| measured.remaining(pool));
        validate(&properties, &mut measured, &|| false).unwrap();
        let bytes = initial[0] - measured.remaining(Resource::Bytes);
        let steps = initial[1] - measured.remaining(Resource::Steps);
        assert!(bytes > 0 && steps > 0);
        assert_eq!(measured.remaining(Resource::Rows), 1_000_000);
        let mut exact = ResourceBudget::new(bytes, 0, steps);
        validate(&properties, &mut exact, &|| false).unwrap();
        assert_eq!(exact.remaining(Resource::Bytes), 0);
        assert_eq!(exact.remaining(Resource::Steps), 0);
        for (allowance, pool) in [
            (ResourceBudget::new(bytes - 1, 0, steps), Resource::Bytes),
            (ResourceBudget::new(bytes, 0, steps - 1), Resource::Steps),
        ] {
            let mut allowance = allowance;
            assert!(
                matches!(validate(&properties, &mut allowance, &|| false), Err(InspectError::Limit(error)) if error.resource == pool)
            );
        }
        let polls = Cell::new(0_usize);
        validate(&properties, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        let terminal = polls.get();
        for stop in [1, terminal / 2, terminal] {
            polls.set(0);
            assert_eq!(
                validate(&properties, &mut budget(), &|| {
                    polls.set(polls.get() + 1);
                    polls.get() == stop
                }),
                Err(InspectError::Cancelled)
            );
            assert_eq!(polls.get(), stop);
        }
    }
}
