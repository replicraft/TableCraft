//! Real retained owned keys exercise the kernel without extending admission.
//! These INDEX-only controls do not certify placement of a new DATA record.
use super::*;
use crate::table::indexes::{
    key::{KeyProfile, KeyWork},
    mutation::{profile::SingleProfile, retained::Preserved},
};
use tablecraft_schema::ScalarType;

#[test]
fn owned_retained_keys_route_repack_and_preserve_the_numeric_image_policy() {
    for code in [5, 6] {
        for aliases in [false, true] {
            let before = routing::source(code, aliases);
            let checked = routing::checked(&before);
            let image = capture(&before);
            let profile = Preserved(SingleProfile {
                column: 1,
                kind: NumericKey::new(ScalarType::Int32).unwrap(),
                descending: false,
            });
            let mut input = image.reader();
            let mut allowance = budget();
            let mut reader = Reader {
                source: &mut input,
                header: image.catalog().header(),
                budget: &mut allowance,
                cancelled: &|| false,
                reads: 0,
            };
            let original = source::capture::<_, ENTRY_BUFFER>(
                &image,
                checked.indexes()[0].index(),
                10,
                &profile,
                &mut reader,
            )
            .unwrap();
            let numeric = [
                KeyRef {
                    key: Some(10),
                    row: 12 << 8,
                },
                KeyRef {
                    key: Some(15),
                    row: (12 << 8) | 3,
                },
                KeyRef {
                    key: Some(20),
                    row: (12 << 8) | 1,
                },
                KeyRef {
                    key: Some(30),
                    row: (12 << 8) | 2,
                },
            ];
            let mut keys = Vec::new();
            source::reserve(&mut keys, numeric.len(), &mut reader).unwrap();
            for entry in &numeric {
                let mut wire = vec![127];
                wire.extend(((entry.key.unwrap() as u32) ^ 0x80000000).to_be_bytes());
                let (key, consumed) = profile
                    .decode(
                        &wire,
                        &mut KeyWork {
                            budget: reader.budget,
                            cancelled: reader.cancelled,
                        },
                    )
                    .unwrap();
                assert_eq!(consumed, 5);
                keys.push(KeyRef {
                    key,
                    row: entry.row,
                });
            }
            let layout =
                layout::prepare::<_, ENTRY_BUFFER>(&image, &original, &keys, &profile, &mut reader)
                    .unwrap();
            assert!(layout.fresh.is_empty());
            let numbers = layout
                .nodes
                .iter()
                .map(|node| match node.origin {
                    layout::Origin::Existing(number) => number,
                    _ => panic!("fitting original nodes must retain their identities"),
                })
                .collect::<Vec<_>>();
            let mut plan = Plan::with_policy(State::new(&image, 10), AllocationPolicy::Qualified);
            write::emit::<_, ENTRY_BUFFER>(
                write::Operation {
                    image: &image,
                    keys: &keys,
                    profile: &profile,
                    layout: &layout,
                    numbers: &numbers,
                },
                &mut plan,
                &mut reader,
            )
            .unwrap();
            assert!(plan.added.is_empty());
            assert!(plan.promotions.is_empty());
            assert!(plan.append.map_edits.is_empty());
            assert_eq!(plan.patches.len(), 1);
            assert_eq!(plan.patches[0].offset, 15 * 4096);
            let mut expected = before.clone();
            expected[15 * 4096..16 * 4096]
                .copy_from_slice(&routing::leaf(&before[15 * 4096..16 * 4096], &numeric[..3]));
            let mut actual = before.clone();
            for patch in &plan.patches {
                let end = patch.offset + patch.before.len();
                assert_eq!(&before[patch.offset..end], patch.before);
                actual[patch.offset..end].copy_from_slice(&patch.after);
            }
            let difference = actual
                .iter()
                .zip(&expected)
                .position(|(left, right)| left != right);
            assert_eq!(actual.len(), expected.len());
            assert!(
                difference.is_none(),
                "first INDEX-only byte difference at {difference:?}"
            );
        }
    }
}
