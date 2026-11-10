//! Literal prefix-free ascending LONG membership refers to physical logical roots.
const PAGE: usize = 4096;
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

pub(super) fn capture(bytes: &[u8], definition_page: u32) -> Vec<Entry<'_>> {
    let definition = definition_page as usize * PAGE;
    let data = (definition_page as usize + 1) * PAGE;
    let index_page = (definition_page as usize + 2) * PAGE;
    let live =
        u32::from_le_bytes(bytes[definition + 16..definition + 20].try_into().unwrap()) as usize;
    assert_eq!(word(bytes, index_page + 24), 0);
    assert_eq!(
        usize::from(word(bytes, index_page + 2)),
        PAGE - ENTRIES - live * KEY_BYTES
    );
    let mut entries = Vec::new();
    let mut bitmap = vec![0; ENTRIES - BITMAP];
    let mut previous = None;
    for index in 0..live {
        let at = index_page + ENTRIES + index * KEY_BYTES;
        let key = &bytes[at..at + KEY_BYTES];
        assert_eq!(key[0], 127);
        let id = (u32::from_be_bytes(key[1..5].try_into().unwrap()) ^ 0x8000_0000) as i32;
        assert!(previous.is_none_or(|prior| prior < id));
        previous = Some(id);
        let reference = u32::from_be_bytes(key[5..].try_into().unwrap());
        assert_eq!(reference >> 8, definition_page + 1);
        let root = reference as usize & 255;
        assert!(root < usize::from(word(bytes, data + 12)));
        let raw = word(bytes, data + DIRECTORY + 2 * root);
        assert_eq!(raw & !(OFFSET_MASK | FORWARDED), 0);
        assert!(!entries.iter().any(|entry: &Entry<'_>| entry.root == root));
        entries.push(Entry { root, bytes: key });
        let end = (index + 1) * KEY_BYTES;
        bitmap[end / 8] |= 1 << (end % 8);
    }
    assert_eq!(&bytes[index_page + BITMAP..index_page + ENTRIES], bitmap);
    entries
}
