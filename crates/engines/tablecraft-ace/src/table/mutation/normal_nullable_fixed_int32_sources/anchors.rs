//! First original image and nullable user-request pins, before producer calls.
use sha2::{Digest, Sha256};
use tablecraft_testkit::access::canonical_nullable_fixed_int32_before_change::{
    self as model, Recipe,
};

fn text(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value.as_bytes());
}

fn hex(hash: Sha256) -> String {
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn nullable_int32_images_and_original_requests_keep_first_export_pins() {
    let mut images = Sha256::new();
    let mut requests = Sha256::new();
    let mut bytes = 0;
    for header in [5, 6] {
        for recipe in [Recipe::Activate, Recipe::Clear] {
            let case = model::source(header, recipe);
            for hash in [&mut images, &mut requests] {
                hash.update([header]);
                text(hash, recipe.id());
            }
            for image in [&case.before, &case.expected] {
                images.update((image.len() as u64).to_le_bytes());
                images.update(image);
                bytes += image.len();
            }
            text(&mut requests, "Items");
            text(&mut requests, "Id");
            requests.update(0_u64.to_le_bytes());
            for value in [case.expected_id, case.replacement_id] {
                requests.update([u8::from(value.is_some())]);
                if let Some(value) = value {
                    requests.update(value.to_le_bytes());
                }
            }
        }
    }
    assert_eq!(bytes, 294912);
    assert_eq!(
        hex(images),
        "cf95ee06fe37d7e4ac665d59a1b689263150500cdb03d92d70bcc4b65cde394f"
    );
    assert_eq!(
        hex(requests),
        "c4311a7b508a82d2ded722b15e1c55a43e0d58ef998ae6e40fc6cf2210472524"
    );
}
