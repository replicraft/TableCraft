use super::super::inspect::payload_fixture;
use tablecraft_testkit::access as raw;
use tablecraft_testkit::access::wide_properties as fixture;

pub(crate) const PAGE: usize = 4096;
pub(crate) const ORIGINAL_ROWS: [[i32; 2]; 3] = [[1, 10], [2, 20], [3, 30]];

/// Build native catalog-owned MR2/XML plus actual original Items records.
/// Existing raw payload/framing owners remain the only source builders.
pub(crate) fn source(code: u8, event: &str, body: &str) -> (Vec<u8>, String, Vec<u8>) {
    let xml = format!(
        "<DataMacro xmlns=\"http://schemas.microsoft.com/office/accessservices/2009/11/application\" Event=\"{event}\">{body}</DataMacro>"
    );
    let bag = fixture::bag(&[
        fixture::names(&["StoredGuard"]),
        fixture::group(3, &[fixture::property(0, 1, 12, &fixture::text(&xml))]),
    ]);
    let mut bytes =
        payload_fixture::database(code, &[None, None, Some(payload_fixture::inline(&bag))]);

    // The generic payload fixture intentionally creates an object of another
    // native type. This source explicitly declares a genuine local Items table.
    let page = &bytes[4 * PAGE..5 * PAGE];
    let start = usize::from(u16::from_le_bytes([page[18], page[19]])) & 0x0fff;
    let end = usize::from(u16::from_le_bytes([page[16], page[17]])) & 0x0fff;
    let mut selected = page[start..end].to_vec();
    raw::put16(&mut selected, 10, 1);
    let records = [
        (0, raw::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects")),
        (
            0,
            raw::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        ),
        (0, selected),
    ];
    bytes[4 * PAGE..5 * PAGE].copy_from_slice(&raw::data_page(2, &records));
    raw::put32(&mut bytes, 2 * PAGE + 16, 3);

    (bytes, xml, bag)
}

pub(crate) fn original_record(ordinal: usize) -> Vec<u8> {
    let [id, score] = ORIGINAL_ROWS[ordinal];
    let mut record = 2_u16.to_le_bytes().to_vec();
    record.extend(id.to_le_bytes());
    record.extend(score.to_le_bytes());
    record.push(3);
    record
}
