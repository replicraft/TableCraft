//! Explicit six-byte ownership-list framing of historical literal fixtures.
use super::model::Case;

const EMPTY_LISTS: [(usize, usize); 3] = [(2, 398), (7, 449), (10, 663)];

pub fn framed(mut case: Case) -> Case {
    for bytes in [&mut case.before, &mut case.expected] {
        let original = bytes.clone();
        for (page, tail) in EMPTY_LISTS {
            let offset = page * 4096 + tail;
            assert_eq!(&bytes[offset..offset + 12], &[0; 12]);
            bytes[offset..offset + 2].copy_from_slice(&[255, 255]);
        }
        let differences = bytes
            .iter()
            .zip(&original)
            .enumerate()
            .filter_map(|(offset, (after, before))| (after != before).then_some(offset))
            .collect::<Vec<_>>();
        let allowed = EMPTY_LISTS
            .iter()
            .flat_map(|&(page, tail)| [page * 4096 + tail, page * 4096 + tail + 1])
            .collect::<Vec<_>>();
        assert_eq!(differences, allowed);
    }

    case
}
