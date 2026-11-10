//! Independent retained wire framing and original complete-image bindings.
use super::{Error, Intent, literal};
use tablecraft_core::limits::ResourceBudget;

#[cfg(feature = "ace16-create")]
mod recovery;

fn work() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 2_000_000_000)
}

fn wire(before: &[u8], short_text: bool) -> Vec<u8> {
    let column = if short_text { "NextText" } else { "NextNumber" };
    let mut bytes = literal(before, "Table1", column);
    bytes[..8].copy_from_slice(if short_text { b"TCAC0011" } else { b"TCAC0010" });
    bytes[12..14].copy_from_slice(&(if short_text { 11_u16 } else { 10_u16 }).to_le_bytes());
    if short_text {
        bytes[14..16].copy_from_slice(&2_u16.to_le_bytes());
        bytes[16..18].copy_from_slice(&255_u16.to_le_bytes());
        bytes[18..20].copy_from_slice(&1_u16.to_le_bytes());
    }
    bytes
}

#[test]
fn retained_wire_is_closed_to_original_languages_and_validates_before_allocation() {
    let before = vec![0; 4096];
    for text in [false, true] {
        let good = wire(&before, text);
        let intent = Intent::decode_retained(&good, &mut work(), &|| false).unwrap();
        assert!(intent.is_retained());
        assert_eq!(intent.encode(&mut work(), &|| false).unwrap(), good);
        intent
            .verify_before(&before, &mut work(), &|| false)
            .unwrap();
        assert!(matches!(
            Intent::decode(&good, &mut work(), &|| false),
            Err(Error::InvalidRecord)
        ));
        assert!(matches!(
            Intent::decode_short_text(&good, &mut work(), &|| false),
            Err(Error::InvalidRecord)
        ));
        for at in [0, 8, 12, 14, 20, 60, 62] {
            let mut malformed = good.clone();
            malformed[at] ^= 1;
            // Changing native byte length by one breaks page alignment before names are owned.
            assert!(matches!(
                Intent::decode_retained(
                    &malformed,
                    &mut ResourceBudget::new(0, 100, 100_000),
                    &|| false
                ),
                Err(Error::InvalidRecord | Error::Binding(_))
            ));
        }
        for (at, invalid) in [(16, 256_u16), (18, 2_u16)] {
            let mut malformed = good.clone();
            malformed[at..at + 2].copy_from_slice(&invalid.to_le_bytes());
            assert!(matches!(
                Intent::decode_retained(
                    &malformed,
                    &mut ResourceBudget::new(0, 100, 100_000),
                    &|| false
                ),
                Err(Error::InvalidRecord)
            ));
        }
        assert!(matches!(
            Intent::decode_retained(&good, &mut work(), &|| true),
            Err(Error::Inspect(crate::InspectError::Cancelled))
        ));
        assert!(matches!(
            Intent::decode_retained(&good, &mut ResourceBudget::new(0, 100, 100_000), &|| false),
            Err(Error::Inspect(crate::InspectError::Limit(_)))
        ));
    }
    let old = literal(&before, "Table1", "Bonus");
    assert!(matches!(
        Intent::decode_retained(&old, &mut work(), &|| false),
        Err(Error::InvalidRecord)
    ));
}

#[test]
fn retained_binding_accepts_native_lengths_above_the_original_image_profile_only() {
    // Inert bytes establish data/framing capacity, not native editing eligibility.
    let mut before = vec![0; 64 * 1024 * 1024 + 4096];
    let good = wire(&before, false);
    let intent = Intent::decode_retained(&good, &mut work(), &|| false).unwrap();
    intent
        .verify_before(&before, &mut work(), &|| false)
        .unwrap();
    before[64 * 1024 * 1024] = 1;
    assert!(matches!(
        intent.verify_before(&before, &mut work(), &|| false),
        Err(Error::BeforeMismatch)
    ));
    let mut oversized = good;
    oversized[20..28].copy_from_slice(&(2_u64 * 1024 * 1024 * 1024 + 4096).to_le_bytes());
    assert!(matches!(
        Intent::decode_retained(
            &oversized,
            &mut ResourceBudget::new(0, 100, 100_000),
            &|| false
        ),
        Err(Error::Binding(_))
    ));
    let old = literal(&before, "Table1", "NextNumber");
    assert!(matches!(
        Intent::decode(&old, &mut work(), &|| false),
        Err(Error::Binding(_))
    ));
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn recognized_retained_data_cannot_qualify_mutation_without_creation_support() {
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;

    let before = vec![0; 4096];
    let encoded = wire(&before, false);
    let mut target = MemoryStorage::new(before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal,
            &before,
            &before,
            &mut work(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal, &mut work(), &|| false).unwrap();
    let intent = Intent::decode_retained(&encoded, &mut work(), &|| false).unwrap();
    assert!(matches!(
        intent.qualify_retained_recovery(checked, CatalogScope::new(1515), &mut work(), &|| false),
        Err(Error::Field(crate::CurrentFieldAdditionError::Source))
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
