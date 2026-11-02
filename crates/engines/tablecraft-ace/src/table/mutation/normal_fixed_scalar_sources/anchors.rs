//! Original image/request pins frozen before normal typed producer execution.
use sha2::{Digest, Sha256};
use tablecraft_testkit::fixtures::access::canonical_fixed_before_change::{self as model, Cell};

fn integer(hash: &mut Sha256, value: usize) {
    hash.update((value as u64).to_le_bytes());
}

fn text(hash: &mut Sha256, value: &str) {
    integer(hash, value.len());
    hash.update(value.as_bytes());
}

fn cell(hash: &mut Sha256, value: Cell) {
    match value {
        Cell::Null => hash.update([0]),
        Cell::Boolean(value) => hash.update([1, u8::from(value)]),
        Cell::Int32(value) => {
            hash.update([2]);
            hash.update(value.to_le_bytes());
        }
        Cell::Int64(value) => {
            hash.update([3]);
            hash.update(value.to_le_bytes());
        }
        Cell::Currency(value) => {
            hash.update([4]);
            hash.update(value.to_le_bytes());
        }
        Cell::Date(bits) => {
            hash.update([5]);
            hash.update(bits.to_le_bytes());
        }
    }
}

fn hex(hash: Sha256) -> String {
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn fixed_scalar_images_and_typed_requests_keep_first_export_pins() {
    let mut images = Sha256::new();
    let mut requests = Sha256::new();
    let mut bytes = 0;
    let mut views = 0;

    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            for hash in [&mut images, &mut requests] {
                hash.update([header]);
                text(hash, recipe.id());
            }

            for image in [Some(&case.before), case.expected.as_ref()] {
                images.update([u8::from(image.is_some())]);
                if let Some(image) = image {
                    integer(&mut images, image.len());
                    images.update(image);
                    bytes += image.len();
                    views += 1;
                }
            }

            text(&mut requests, "Items");
            integer(&mut requests, case.groups.len());
            for group in &case.groups {
                let field = model::FIELDS
                    .iter()
                    .find(|field| field.0 == group.column)
                    .unwrap();
                text(&mut requests, field.1);
                integer(&mut requests, group.column);
                integer(&mut requests, group.updates.len());
                for update in &group.updates {
                    integer(&mut requests, update.ordinal);
                    cell(&mut requests, update.expected);
                    cell(&mut requests, update.replacement);
                }
            }
        }
    }

    assert_eq!((views, bytes), (66, 2433024));
    assert_eq!(
        hex(images),
        "338148282c163fc11db5d155711eaa43716e5275864e505c02cd7e98d3ce09f1"
    );
    assert_eq!(
        hex(requests),
        "d094c38d4a992be20e5f1114910ce10089f15ecc8e893b35d2b58e8bf913a297"
    );
}
