//! Additive rich catalog carriers; original fixture owners are never changed.
use super::{int32_model, raw, rich};

const PAGE: usize = 4096;
const CATALOG_DATA: usize = 4;
const NAMESPACE: &str = "http://schemas.microsoft.com/office/accessservices/2009/11/application";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Recipe {
    RichReadGuard,
    Int32Sequential,
    Int32DerivedOnly,
    Int32LateRaise,
    DormantIndexedTarget,
    DormantDependentTarget,
    OpaqueFieldRead,
    WideFieldRead,
    BodyPresent,
    ModulePresent,
    OpaqueExtra,
    UnselectedProperty,
    ChildPresent,
    DormantDelete,
    CounterRead,
    CounterTarget,
    ComplexCurrentRead,
    ComplexOldRead,
}

pub(crate) const RECIPES: [Recipe; 18] = [
    Recipe::RichReadGuard,
    Recipe::Int32Sequential,
    Recipe::Int32DerivedOnly,
    Recipe::Int32LateRaise,
    Recipe::DormantIndexedTarget,
    Recipe::DormantDependentTarget,
    Recipe::OpaqueFieldRead,
    Recipe::WideFieldRead,
    Recipe::BodyPresent,
    Recipe::ModulePresent,
    Recipe::OpaqueExtra,
    Recipe::UnselectedProperty,
    Recipe::ChildPresent,
    Recipe::DormantDelete,
    Recipe::CounterRead,
    Recipe::CounterTarget,
    Recipe::ComplexCurrentRead,
    Recipe::ComplexOldRead,
];

pub(crate) struct Case {
    pub before: Vec<u8>,
    pub original: Vec<u8>,
    pub int32: bool,
    pub properties: Vec<(String, String)>,
    pub selection: Vec<usize>,
    pub extra: Vec<u8>,
    pub body: Option<Vec<u8>>,
    pub module: Option<Vec<u8>>,
    pub child: bool,
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn catalog_records(bytes: &[u8]) -> Vec<Vec<u8>> {
    let page = &bytes[CATALOG_DATA * PAGE..(CATALOG_DATA + 1) * PAGE];
    assert_eq!(&page[..2], &[1, 1]);
    assert_eq!(&page[4..8], &2_u32.to_le_bytes());
    let count = word(page, 12);
    assert_eq!(count, 6);
    (0..count)
        .map(|slot| {
            let start = word(page, 14 + slot * 2);
            let end = if slot == 0 {
                PAGE
            } else {
                word(page, 12 + slot * 2)
            };
            assert_eq!(start & 0xe000, 0);
            assert!(14 + count * 2 <= start && start < end && end <= PAGE);
            page[start..end].to_vec()
        })
        .collect()
}

fn inline(payload: &[u8]) -> Vec<u8> {
    let mut output = (0x8000_0000_u32 | u32::try_from(payload.len()).unwrap())
        .to_le_bytes()
        .to_vec();
    output.extend([0; 8]);
    output.extend(payload);
    output
}

fn event(name: &str, body: &str) -> String {
    format!("<DataMacro Event=\"{name}\"><Statements>{body}</Statements></DataMacro>")
}

fn set_field(name: &str, value: &str) -> String {
    format!(
        "<Action Name=\"SetField\"><Argument Name=\"Field\">{name}</Argument><Argument Name=\"Value\">{value}</Argument></Action>"
    )
}

fn raise_if(condition: &str) -> String {
    format!(
        "<ConditionalBlock><If><Condition>{condition}</Condition><Statements><Action Name=\"RaiseError\"><Argument Name=\"Number\">17</Argument><Argument Name=\"Description\">rejected</Argument></Action></Statements></If></ConditionalBlock>"
    )
}

fn program(recipe: Recipe) -> String {
    let body = match recipe {
        Recipe::Int32Sequential => format!(
            "{}{}",
            set_field("Large_Number", "[Large_Number]+[ID]"),
            set_field("Large_Number", "[Large_Number]+[Old].[Large_Number]")
        ),
        Recipe::CounterRead | Recipe::Int32DerivedOnly => set_field("Large_Number", "[ID]"),
        Recipe::Int32LateRaise => format!(
            "{}{}",
            set_field("Large_Number", "[ID]"),
            raise_if("[ID]=2")
        ),
        Recipe::DormantIndexedTarget => format!(
            "<ConditionalBlock><If><Condition>False</Condition><Statements>{}</Statements></If></ConditionalBlock>",
            set_field("ID", "1")
        ),
        Recipe::DormantDependentTarget => format!(
            "<ConditionalBlock><If><Condition>False</Condition><Statements>{}</Statements></If></ConditionalBlock>",
            set_field("Number", "1")
        ),
        Recipe::CounterTarget => set_field("ID", "1"),
        Recipe::ComplexCurrentRead => raise_if("[Attachment_Field]=0"),
        Recipe::ComplexOldRead => raise_if("[Old].[Attachment_Field]=0"),
        Recipe::OpaqueFieldRead => raise_if("[Calculated]=0"),
        Recipe::WideFieldRead => raise_if("[Currency_Field]=0"),
        _ => raise_if("[ID]&lt;0"),
    };
    let mut definitions = event("BeforeChange", &body);
    if recipe == Recipe::DormantDelete {
        definitions.push_str(&event("BeforeDelete", &raise_if("[Old].[ID]=2")));
    }
    format!("<DataMacros xmlns=\"{NAMESPACE}\">{definitions}</DataMacros>")
}

fn selected_record(
    original: &[u8],
    extra: &[u8],
    body: Option<&[u8]>,
    module: Option<&[u8]>,
) -> Vec<u8> {
    // Existing rich LvProp is a separate native catalog role. It must never be
    // replaced by the macro bag or mistaken for a declaration of LvExtra.
    assert_eq!(word(original, 0), 6);
    assert_eq!(original.last(), Some(&0x3f));
    let count_at = original.len() - 3;
    assert_eq!(word(original, count_at), 2);
    let offsets = (0..3)
        .map(|index| word(original, count_at - (index + 1) * 2))
        .collect::<Vec<_>>();
    assert_eq!(offsets[0], 16);
    assert_eq!(offsets[2], count_at - 6);
    let name = &original[offsets[0]..offsets[1]];
    assert_eq!(name, b"\xff\xfeItems");
    let properties = &original[offsets[1]..offsets[2]];
    assert_eq!(&properties[12..16], b"MR2\0");
    assert_eq!(
        &properties[..4],
        &(0x8000_0000_u32 | u32::try_from(properties.len() - 12).unwrap()).to_le_bytes()
    );
    assert_eq!(&properties[4..12], &[0; 8]);

    let mut row = original[..16].to_vec();
    raw::put16(&mut row, 0, 9);
    let values = [
        name.to_vec(),
        properties.to_vec(),
        body.map(inline).unwrap_or_default(),
        inline(extra),
        module.map(inline).unwrap_or_default(),
    ];
    let mut new_offsets = vec![16_u16];
    for value in values {
        row.extend(value);
        new_offsets.push(u16::try_from(row.len()).unwrap());
    }
    for offset in new_offsets.into_iter().rev() {
        row.extend(offset.to_le_bytes());
    }
    row.extend(5_u16.to_le_bytes());
    // Real field numbers: Name2/LvProp5/Lv6/LvExtra7/LvModule8.
    row.push(0xbf | if body.is_some() { 0x40 } else { 0 });
    row.push(u8::from(module.is_some()));
    row
}

pub(crate) fn source(code: u8, recipe: Recipe) -> Case {
    assert!(matches!(code, 5 | 6));
    let int32 = recipe != Recipe::RichReadGuard;
    let original = if int32 {
        int32_model::source(code, 101).before
    } else {
        let mut bytes = rich::source().before;
        bytes[20] = code;
        bytes
    };
    assert_eq!(original.len(), 32 * PAGE);
    assert_eq!(original[20], code);
    if matches!(recipe, Recipe::CounterRead | Recipe::CounterTarget) {
        let definition = 10 * PAGE;
        assert_eq!(
            u32::from_le_bytes(
                original[definition + 51..definition + 55]
                    .try_into()
                    .unwrap()
            ),
            3
        );
        let field = definition + 63 + 3 * 12;
        assert_eq!(original[field], 4);
        assert_eq!(&original[field + 5..field + 7], &0_u16.to_le_bytes());
        assert_eq!(&original[field + 15..field + 17], &[7, 0]);
        assert_eq!(&original[field + 23..field + 25], &4_u16.to_le_bytes());
        // The actual original ID already carries fixed/updatable/AutoNumber
        // flags. Reuse that declaration and its stored keys without any patch.
    }
    let mut records = catalog_records(&original);
    let xml = program(recipe);
    let mut properties = vec![("StoredGuard".to_owned(), xml)];
    if recipe == Recipe::UnselectedProperty {
        properties.push(("UnselectedGuard".to_owned(), program(Recipe::RichReadGuard)));
    }
    let borrowed = properties
        .iter()
        .map(|(name, xml)| (name.as_str(), xml.as_str()))
        .collect::<Vec<_>>();
    let event_properties = raw::wide_properties::event_properties(&borrowed);
    let extra = if recipe == Recipe::OpaqueExtra {
        b"opaque executable-potential bytes".to_vec()
    } else {
        event_properties
    };
    let body = (recipe == Recipe::BodyPresent).then(|| b"opaque body".to_vec());
    let module = (recipe == Recipe::ModulePresent).then(|| b"opaque module".to_vec());
    records[2] = selected_record(&records[2], &extra, body.as_deref(), module.as_deref());
    let child = recipe == Recipe::ChildPresent;
    if child {
        let mut record = raw::record(4001, 10, -32761_i16, 0, b"\xff\xfeChild");
        assert_eq!(word(&record, 0), 5);
        // Absent streams remain observed on the existing catalog declaration;
        // the real direct child still blocks this engineering event profile.
        raw::put32(&mut record, 6, 10);
        records.push(record);
    }
    let rows = records
        .into_iter()
        .map(|record| (0, record))
        .collect::<Vec<_>>();
    let payload_bytes = rows.iter().map(|(_, row)| row.len()).sum::<usize>();
    assert!(payload_bytes + 14 + rows.len() * 2 < PAGE);
    let mut before = original.clone();
    before[4 * PAGE..5 * PAGE].copy_from_slice(&raw::data_page(2, &rows));
    raw::put16(
        &mut before,
        4 * PAGE + 2,
        u16::try_from(PAGE - 14 - rows.len() * 2 - payload_bytes).unwrap(),
    );
    if child {
        let count_at = 2 * PAGE + 16;
        assert_eq!(&original[count_at..count_at + 4], &6_u32.to_le_bytes());
        raw::put32(&mut before, count_at, 7);
        assert_eq!(&before[..count_at], &original[..count_at]);
        assert_eq!(
            &before[count_at + 4..4 * PAGE],
            &original[count_at + 4..4 * PAGE]
        );
    } else {
        assert_eq!(&before[..4 * PAGE], &original[..4 * PAGE]);
    }
    assert_eq!(&before[5 * PAGE..], &original[5 * PAGE..]);
    Case {
        before,
        original,
        int32,
        properties,
        selection: vec![0],
        extra,
        body,
        module,
        child,
    }
}
