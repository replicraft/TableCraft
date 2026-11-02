//! Small original stored ZIP/OPC template, independent of format readers/writers.

const TYPES: &str = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="text/xml"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/></Types>"#;
const ROOT: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="start" Type="http://schemas.microsoft.com/office/access/2005/04/template/start" Target="template/main.xml"/></Relationships>"#;
const UTF8_FLAG: u16 = 0x0800;
const ZIP_VERSION: u16 = 20;
const DOS_FIRST_DATE: u16 = 33;
const CRC_POLYNOMIAL: u32 = 0xedb8_8320;

/// A valid inert package with no object declarations or executable content.
/// Useful for proving unsupported operations refuse after ordinary OPC capture
/// without pretending that capture performs no package-byte I/O.
pub fn empty_template() -> Vec<u8> {
    let parts = [
        ("[Content_Types].xml", TYPES),
        ("_rels/.rels", ROOT),
        ("template/main.xml", "<Template/>"),
    ];
    let mut bytes = Vec::new();
    let mut directory = Vec::new();
    for (name, body) in parts {
        let start = bytes.len() as u32;
        let length = body.len() as u32;
        let checksum = crc32(body.as_bytes());
        bytes.extend_from_slice(b"PK\x03\x04");
        for value in [ZIP_VERSION, UTF8_FLAG, 0, 0, DOS_FIRST_DATE] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in [checksum, length, length] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(body.as_bytes());

        directory.extend_from_slice(b"PK\x01\x02");
        for value in [ZIP_VERSION, ZIP_VERSION, UTF8_FLAG, 0, 0, DOS_FIRST_DATE] {
            directory.extend_from_slice(&value.to_le_bytes());
        }
        for value in [checksum, length, length] {
            directory.extend_from_slice(&value.to_le_bytes());
        }
        for value in [name.len() as u16, 0, 0, 0, 0] {
            directory.extend_from_slice(&value.to_le_bytes());
        }
        directory.extend_from_slice(&0u32.to_le_bytes());
        directory.extend_from_slice(&start.to_le_bytes());
        directory.extend_from_slice(name.as_bytes());
    }
    let start = bytes.len() as u32;
    let length = directory.len() as u32;
    bytes.extend_from_slice(&directory);
    bytes.extend_from_slice(b"PK\x05\x06");
    for value in [0u16, 0, 3, 3] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(&start.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..u8::BITS {
            crc = (crc >> 1) ^ if crc & 1 == 0 { 0 } else { CRC_POLYNOMIAL };
        }
    }
    !crc
}
