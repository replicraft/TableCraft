//! Original image/request pins frozen before normal typed producer execution.
use sha2::{Digest, Sha256};
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_before_change::Cell, canonical_fixed_sql_before_change as model,
};

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
fn fixed_sql_images_and_original_requests_keep_first_export_pins() {
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

            for image in [&case.before, &case.expected] {
                integer(&mut images, image.len());
                images.update(image);
                bytes += image.len();
                views += 1;
            }

            text(&mut requests, case.sql);
            text(&mut requests, case.reversed_sql);
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

    assert_eq!((views, bytes), (20, 737280));
    assert_eq!(
        hex(images),
        "f991a15d728d36ee773fcf76f79726df0f3d70900559ae15a0455529249f46b9"
    );
    assert_eq!(
        hex(requests),
        "4eddabbf5877a455388da35b43b7294422bd0d666f2cd04b208eed8214aaf451"
    );
}
