//! Literal prefix-free ascending LONG membership refers to physical logical roots.
const PAGE: usize = 4096;
const DEFINITION: usize = 7 * PAGE;
const DATA: usize = 8 * PAGE;
const INDEX: usize = 9 * PAGE;
const DIRECTORY: usize = 14;
const ENTRIES: usize = 480;
const BITMAP: usize = 27;
const KEY_BYTES: usize = 9;
const OFFSET_MASK: u16 = 0x1fff;
const FORWARDED: u16 = 0x4000;

pub(super) struct Entry<'a> {
    pub root: usize,
    pub bytes: &'a [u8],
}

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}

pub(super) fn capture(bytes: &[u8]) -> Vec<Entry<'_>> {
    let live =
        u32::from_le_bytes(bytes[DEFINITION + 16..DEFINITION + 20].try_into().unwrap()) as usize;
    assert_eq!(word(bytes, INDEX + 24), 0);
    assert_eq!(
        usize::from(word(bytes, INDEX + 2)),
        PAGE - ENTRIES - live * KEY_BYTES
    );
    let mut entries = Vec::new();
    let mut bitmap = vec![0; ENTRIES - BITMAP];
    let mut previous = None;
    for index in 0..live {
        let at = INDEX + ENTRIES + index * KEY_BYTES;
        let key = &bytes[at..at + KEY_BYTES];
        assert_eq!(key[0], 127);
        let id = (u32::from_be_bytes(key[1..5].try_into().unwrap()) ^ 0x8000_0000) as i32;
        assert!(previous.is_none_or(|prior| prior < id));
        previous = Some(id);
        let reference = u32::from_be_bytes(key[5..].try_into().unwrap());
        assert_eq!(reference >> 8, 8);
        let root = reference as usize & 255;
        assert!(root < usize::from(word(bytes, DATA + 12)));
        let raw = word(bytes, DATA + DIRECTORY + 2 * root);
        assert_eq!(raw & !(OFFSET_MASK | FORWARDED), 0);
        assert!(!entries.iter().any(|entry: &Entry<'_>| entry.root == root));
        entries.push(Entry { root, bytes: key });
        let end = (index + 1) * KEY_BYTES;
        bitmap[end / 8] |= 1 << (end % 8);
    }
    assert_eq!(&bytes[INDEX + BITMAP..INDEX + ENTRIES], bitmap);
    entries
}
