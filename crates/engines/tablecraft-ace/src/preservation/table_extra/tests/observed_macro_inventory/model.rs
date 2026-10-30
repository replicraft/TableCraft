use super::super::{fixture, guard_sources::model as original, inspect::payload_fixture};

#[derive(Clone, Copy, Debug)]
pub(super) enum Case {
    NonstandardEventProperty,
    NamedProgram,
    EventAndNamed,
    NamesOnly,
    OpaqueExtra,
    OpaqueGroup,
    MalformedXml,
    NonTextProperty,
    MultipleEnvelopes,
    UnfamiliarEvent,
}

pub(super) const CASES: [Case; 10] = [
    Case::NonstandardEventProperty,
    Case::NamedProgram,
    Case::EventAndNamed,
    Case::NamesOnly,
    Case::OpaqueExtra,
    Case::OpaqueGroup,
    Case::MalformedXml,
    Case::NonTextProperty,
    Case::MultipleEnvelopes,
    Case::UnfamiliarEvent,
];

pub(super) const EVENT_XML: &str = "<DataMacro xmlns=\"http://schemas.microsoft.com/office/accessservices/2009/11/application\" Event=\"BeforeChange\"><Statements/></DataMacro>";
pub(super) const NAMED_XML: &str = "<DataMacro xmlns=\"http://schemas.microsoft.com/office/accessservices/2009/11/application\" Name=\"RecomputeBalance\"><Statements/></DataMacro>";
pub(super) const WRAPPED_XML: &str = "<DataMacros xmlns=\"http://schemas.microsoft.com/office/accessservices/2009/11/application\"><DataMacro Event=\"BeforeDelete\"><Statements/></DataMacro><DataMacro Name=\"Repair\"><Statements/></DataMacro></DataMacros>";
pub(super) const FUTURE_XML: &str = "<DataMacro xmlns=\"http://schemas.microsoft.com/office/accessservices/2009/11/application\" Event=\"FutureEvent\"><Statements><Action Name=\"FutureAction\"/></Statements></DataMacro>";

pub(super) struct Source {
    pub bytes: Vec<u8>,
    pub bag: Vec<u8>,
}

pub(super) fn source(code: u8, case: Case) -> Source {
    let event = fixture::property(0, 1, 12, &fixture::text(EVENT_XML));
    let named = fixture::property(1, 1, 12, &fixture::text(NAMED_XML));
    let bag = match case {
        Case::NonstandardEventProperty => fixture::bag(&[
            fixture::names(&["ValidateLedger"]),
            fixture::group(3, &[event]),
        ]),
        Case::NamedProgram => fixture::bag(&[
            fixture::names(&["UnusedName", "RecomputeBalance"]),
            fixture::group(4, &[named]),
        ]),
        Case::EventAndNamed => fixture::bag(&[
            fixture::names(&["ValidateLedger", "RecomputeBalance"]),
            fixture::group(3, &[event]),
            fixture::group(4, &[named]),
        ]),
        Case::NamesOnly => fixture::bag(&[fixture::names(&["ValidateLedger", "RecomputeBalance"])]),
        Case::OpaqueExtra => b"UnrecognizedExtra\x01\x02".to_vec(),
        Case::OpaqueGroup => fixture::bag(&[
            fixture::names(&["ValidateLedger"]),
            fixture::block(99, &[0xa1, 0xb2, 0xc3]),
        ]),
        Case::MalformedXml => fixture::bag(&[
            fixture::names(&["ValidateLedger"]),
            fixture::group(3, &[fixture::property(0, 1, 12, &fixture::text("<broken"))]),
        ]),
        Case::NonTextProperty => fixture::bag(&[
            fixture::names(&["ValidateLedger"]),
            fixture::group(3, &[fixture::property(0, 0, 4, &[17, 0, 0, 0])]),
        ]),
        Case::MultipleEnvelopes => fixture::bag(&[
            fixture::names(&["EnvelopeSet"]),
            fixture::group(
                3,
                &[fixture::property(0, 1, 12, &fixture::text(WRAPPED_XML))],
            ),
        ]),
        Case::UnfamiliarEvent => fixture::bag(&[
            fixture::names(&["ValidateLedger"]),
            fixture::group(
                3,
                &[fixture::property(0, 1, 12, &fixture::text(FUTURE_XML))],
            ),
        ]),
    };

    let (local, _, _) = original::source(code, "BeforeChange", "<Statements/>");
    let mut bytes =
        payload_fixture::database(code, &[None, None, Some(payload_fixture::inline(&bag))]);
    let old = item_start(&local);
    let new = item_start(&bytes);
    bytes[new..new + 16].copy_from_slice(&local[old..old + 16]);

    Source { bytes, bag }
}

fn item_start(bytes: &[u8]) -> usize {
    let slot = 4 * 4096 + 18;
    let offset = usize::from(u16::from_le_bytes([bytes[slot], bytes[slot + 1]])) & 0x0fff;
    4 * 4096 + offset
}
