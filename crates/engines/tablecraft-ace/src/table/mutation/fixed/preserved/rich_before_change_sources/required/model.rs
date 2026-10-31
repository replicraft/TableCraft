//! Unchanged pure Required source recipe, reused by native and frontend tests.
use super::{scalar_model, sources as model};

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn bag(bytes: &[u8]) -> (usize, &[u8]) {
    let starts = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(at, prefix)| (prefix == b"MR2\0").then_some(at))
        .collect::<Vec<_>>();
    assert_eq!(starts.len(), 1);
    let start = starts[0];
    assert!(start >= 12);
    let envelope = u32::from_le_bytes(bytes[start - 12..start - 8].try_into().unwrap());
    assert_eq!(envelope & 0xc000_0000, 0x8000_0000);
    assert_eq!(&bytes[start - 8..start], &[0; 8]);
    let length = usize::try_from(envelope & 0x3fff_ffff).unwrap();
    (start, &bytes[start..start + length])
}

fn patch_matching_bag(bytes: &mut [u8], original: &[u8], relative: usize) -> usize {
    let starts = bytes
        .windows(original.len())
        .enumerate()
        .filter_map(|(at, value)| (value == original).then_some(at))
        .collect::<Vec<_>>();
    assert_eq!(starts.len(), 1);
    let patch = starts[0] + relative;
    assert_eq!(bytes[patch], 0);
    bytes[patch] = 1;
    patch
}

pub(crate) struct Case {
    pub source: model::Case,
    pub patch: usize,
    pub original_patch: usize,
    pub repairing: bool,
}

pub(crate) fn source(code: u8, repairing: bool) -> Case {
    let mut source = model::source(
        code,
        if repairing {
            model::Recipe::Int32DerivedOnly
        } else {
            model::Recipe::DormantDelete
        },
    );
    let authority = scalar_model::source(
        code,
        scalar_model::Target::LargeNumber,
        scalar_model::Recipe::RequiredRefusal,
    );
    let (authority_start, required_bag) = bag(&authority.before);
    let (_, original_bag) = bag(&source.original);
    let original_bag = original_bag.to_vec();
    let relative = authority
        .required_patch
        .unwrap()
        .checked_sub(authority_start)
        .unwrap();
    assert_eq!(required_bag.len(), original_bag.len());
    assert_eq!(&required_bag[..relative], &original_bag[..relative]);
    assert_eq!(&required_bag[relative + 1..], &original_bag[relative + 1..]);
    assert_eq!((original_bag[relative], required_bag[relative]), (0, 1));
    // The actual existing helper selects Large_Number's Required entry. Check
    // its raw boolean framing here, and independently re-read the named field
    // with the normal property inspector in the SOURCE control below.
    assert_eq!(
        &original_bag[relative - 8..relative],
        &[9, 0, 1, 1, 10, 0, 1, 0]
    );
    assert_eq!(word(&original_bag, relative - 4), 10);
    let before = source.before.clone();
    let original = source.original.clone();
    let patch = patch_matching_bag(&mut source.before, &original_bag, relative);
    let original_patch = patch_matching_bag(&mut source.original, &original_bag, relative);
    for (old, new, changed) in [
        (&before, &source.before, patch),
        (&original, &source.original, original_patch),
    ] {
        assert_eq!(&old[..changed], &new[..changed]);
        assert_eq!(&old[changed + 1..], &new[changed + 1..]);
    }
    Case {
        source,
        patch,
        original_patch,
        repairing,
    }
}
