use super::super::{guard_sources::model as original, inspect::payload_fixture};
use tablecraft_testkit::access::wide_properties as fixture;

pub(crate) const ALPHA_BODY: &str = "<Statements><Action Name=\"RaiseError\"><Argument Name=\"Number\">71</Argument><Argument Name=\"Description\">alpha</Argument></Action></Statements>";
pub(crate) const BETA_BODY: &str = "<Statements><Action Name=\"RaiseError\"><Argument Name=\"Number\">73</Argument><Argument Name=\"Description\">beta</Argument></Action></Statements>";

#[derive(Clone, Copy, Debug)]
pub(crate) enum Variant {
    Valid,
    SecondOpaque,
    SecondMalformed,
    DuplicateKind,
}

pub(crate) struct Source {
    pub bytes: Vec<u8>,
    pub alpha: String,
    pub beta: String,
    pub bag: Vec<u8>,
}

fn xml(body: &str) -> String {
    format!(
        "<DataMacro xmlns=\"http://schemas.microsoft.com/office/accessservices/2009/11/application\" Event=\"BeforeChange\">{body}</DataMacro>"
    )
}

pub(crate) fn source(code: u8, variant: Variant) -> Source {
    let alpha = xml(ALPHA_BODY);
    let beta = match variant {
        Variant::SecondOpaque => xml("<Statements><Action Name=\"FutureAction\"/></Statements>"),
        Variant::SecondMalformed => "<invalid".into(),
        Variant::Valid | Variant::DuplicateKind => xml(BETA_BODY),
    };
    let bag = if matches!(variant, Variant::DuplicateKind) {
        fixture::bag(&[
            fixture::names(&["GuardA"]),
            fixture::group(3, &[fixture::property(0, 1, 12, &fixture::text(&alpha))]),
            fixture::group(3, &[fixture::property(0, 1, 12, &fixture::text(&beta))]),
        ])
    } else {
        fixture::bag(&[
            fixture::names(&["GuardA", "GuardB"]),
            fixture::group(
                3,
                &[
                    fixture::property(0, 1, 12, &fixture::text(&alpha)),
                    fixture::property(1, 1, 12, &fixture::text(&beta)),
                ],
            ),
        ])
    };

    // Reuse the one actual payload factory. Its object prefix is replaced with
    // the qualified local-table prefix from the original SOURCE owner; the new
    // variable payload/footer remains the independent payload factory's output.
    let (local, _, _) = original::source(code, "BeforeChange", ALPHA_BODY);
    let mut bytes =
        payload_fixture::database(code, &[None, None, Some(payload_fixture::inline(&bag))]);
    let old = item_start(&local);
    let new = item_start(&bytes);
    bytes[new..new + 16].copy_from_slice(&local[old..old + 16]);
    Source {
        bytes,
        alpha,
        beta,
        bag,
    }
}

fn item_start(bytes: &[u8]) -> usize {
    let slot = 4 * 4096 + 18;
    let offset = usize::from(u16::from_le_bytes([bytes[slot], bytes[slot + 1]])) & 0x0fff;
    4 * 4096 + offset
}
