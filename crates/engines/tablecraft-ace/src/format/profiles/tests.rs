use super::*;

fn header(engine: &[u8], code: u8, subversion: u8, encoded: bool) -> Header {
    let mut bytes = [0; 66];
    bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
    bytes[4..20].copy_from_slice(engine);
    bytes[20] = code;
    bytes[21] = subversion;
    bytes[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
    if encoded {
        bytes[62] ^= 1;
    }
    Header::parse(&bytes, 4096 * 3).unwrap()
}

#[test]
fn recognized_headers_do_not_grant_unqualified_read_support() {
    for (engine, code, expected) in [
        (b"Standard Jet DB\0", 0, None),
        (b"Standard Jet DB\0", 1, Some(ReadProfile::Jet4)),
        (b"Standard ACE DB\0", 2, Some(ReadProfile::Ace2007)),
        (b"Standard ACE DB\0", 3, Some(ReadProfile::Ace2010)),
        (b"Standard ACE DB\0", 5, Some(ReadProfile::Ace2016)),
        (b"Standard ACE DB\0", 6, Some(ReadProfile::Ace2019)),
    ] {
        assert_eq!(
            header(engine, code, 0, false).read_profile(),
            expected.ok_or(InspectError::Unsupported)
        );
        assert_eq!(
            header(engine, code, 0, true).read_profile(),
            Err(InspectError::Encoded)
        );
        assert_eq!(
            header(engine, code, 1, false).read_profile(),
            if code == 1 {
                Err(InspectError::Unsupported)
            } else {
                expected.ok_or(InspectError::Unsupported)
            }
        );
    }
}
