use super::{DataMacroXmlNamespace, read::document};
use crate::{InspectError, preservation::table_extra::TableExtraProperty};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};

const NOVEMBER: &str = "http://schemas.microsoft.com/office/accessservices/2009/11/application";
const APRIL: &str = "http://schemas.microsoft.com/office/accessservices/2009/04/application";

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 0, 10_000_000)
}

fn xml(body: &str) -> String {
    format!("<DataMacro xmlns=\"{NOVEMBER}\" Event=\"BeforeChange\">{body}</DataMacro>")
}

#[test]
fn original_event_fact_is_independent_of_native_property_name() {
    let text = xml("<Statements><Action Name=\"FutureAction\"/></Statements>");
    let property = TableExtraProperty {
        name: "ValidateUpdate".into(),
        flags: 1,
        native_type: 12,
        raw: text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        text: Some(text.clone()),
    };
    let result = property
        .inspect_macro_xml(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(result.namespace(), DataMacroXmlNamespace::November2009);
    assert_eq!(result.definitions().len(), 1);
    assert_eq!(result.definitions()[0].event(), Some("BeforeChange"));
    assert_eq!(result.definitions()[0].name(), None);
    assert_eq!(property.name(), "ValidateUpdate");
    assert_eq!(property.text(), Some(text.as_str()));

    let property = TableExtraProperty {
        name: "AfterInsert".into(),
        flags: 0xfe,
        native_type: 0xfe,
        raw: vec![0xff],
        text: None,
    };
    assert_eq!(
        property.inspect_macro_xml(&mut budget(), &|| false).err(),
        Some(InspectError::Unsupported)
    );
}

#[test]
fn wrapped_named_event_prefixes_and_predecoded_declarations_are_explicit() {
    for (namespace, expected) in [
        (APRIL, DataMacroXmlNamespace::April2009),
        (NOVEMBER, DataMacroXmlNamespace::November2009),
    ] {
        let text = format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-16\" standalone=\"no\"?>\
             <a:DataMacros xmlns:a=\"{namespace}\">\
             <a:DataMacro Event=\"AfterInsert\"><a:Statements/></a:DataMacro>\
             <a:DataMacro Name=\"A&amp;B&#x20;C\"><a:Parameters/><a:Statements/></a:DataMacro>\
             </a:DataMacros>"
        );
        let result = document(&text, &mut budget(), &|| false).unwrap();
        assert_eq!(result.namespace(), expected);
        assert_eq!(result.definitions().len(), 2);
        assert_eq!(result.definitions()[0].event(), Some("AfterInsert"));
        assert_eq!(result.definitions()[1].name(), Some("A&B C"));
        assert_eq!(result.definitions()[1].event(), None);
    }
}

#[test]
fn body_text_entities_cdata_and_unicode_qnames_remain_uninterpreted() {
    let text = xml(
        "<Statements xmlns:x=\"urn:opaque\"><Élément x:clé=\"V&amp;&#x41;\">\
         &lt;&gt;&amp;&quot;&apos;&#65;&#x1f600;<![CDATA[<opaque/>]]></Élément></Statements>",
    );
    let result = document(&text, &mut budget(), &|| false).unwrap();
    assert_eq!(result.definitions()[0].event(), Some("BeforeChange"));
    let future = format!("<DataMacro xmlns=\"{NOVEMBER}\" Event=\"FutureEvent\"/>");
    let result = document(&future, &mut budget(), &|| false).unwrap();
    assert_eq!(result.definitions()[0].event(), Some("FutureEvent"));
}

#[test]
fn invalid_envelopes_do_not_become_absence_or_partial_definitions() {
    for text in [
        String::new(),
        format!("<DataMacros xmlns=\"{NOVEMBER}\"/>"),
        format!("<DataMacro xmlns=\"{NOVEMBER}\"/>"),
        format!("<DataMacro xmlns=\"{NOVEMBER}\" Event=\"\"/>"),
        format!("<DataMacro xmlns=\"{NOVEMBER}\" Event=\"X\" Name=\"Y\"/>"),
        xml("<Statements>"),
        xml("</Bad>"),
        format!("{}{}", xml(""), xml("")),
        format!("outside{}", xml("")),
        format!("{}outside", xml("")),
        xml("<DataMacro Event=\"AfterInsert\"/>"),
        xml("<DataMacros/>"),
        xml("<Statements>&unknown;</Statements>"),
        xml("<Statements>&#1;</Statements>"),
        xml("<Statements>bad\0</Statements>"),
        xml("<Statements>bad]]></Statements>"),
        xml("<Statements><!--bad--comment--></Statements>"),
        xml("<1bad/>"),
        xml("<:bad/>"),
        xml("<a:b:c/>"),
        xml("<Statements 1bad=\"x\"/>"),
        xml("<Statements A=\"x\" A=\"y\"/>"),
        xml("<Statements xmlns:a=\"urn:x\" xmlns:b=\"urn:x\" a:V=\"x\" b:V=\"y\"/>"),
        xml("<Statements Value=\"&#1;\"/>"),
        xml("<Statements Value=\"<bad\"/>"),
        format!("<?xml version=\"1.0\" Extra=\"yes\"?>{}", xml("")),
        format!(
            "<?xml version=\"1.0\" standalone=\"yes\" encoding=\"UTF-8\"?>{}",
            xml("")
        ),
        format!("<?xml version=\"1.0\" version=\"1.0\"?>{}", xml("")),
    ] {
        assert!(document(&text, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn external_entities_namespace_changes_and_extensions_refuse_without_resolution() {
    for text in [
        format!(
            "<!DOCTYPE DataMacro SYSTEM 'file:///never-open'>{}",
            xml("")
        ),
        format!(
            "<!DOCTYPE DataMacro [<!ENTITY x SYSTEM 'https://invalid.test/'>]>{}",
            xml("")
        ),
        format!("<?host run='no'?>{}", xml("")),
        xml("<?host run='no'?>"),
        "<DataMacro Event=\"BeforeChange\"/>".to_owned(),
        "<DataMacro xmlns=\"urn:unknown\" Event=\"BeforeChange\"/>".to_owned(),
        xml(&format!("<Statements xmlns=\"{APRIL}\"/>")),
        xml("<x:Statements/>"),
        format!("<DataMacro xmlns=\"{NOVEMBER}\" Event=\"X\" Extra=\"Y\"/>"),
        format!("<DataMacro xmlns=\"{NOVEMBER}\" xmlns:x=\"urn:x\" x:Event=\"X\"/>"),
        format!("<?xml version=\"1.1\"?>{}", xml("")),
        format!("<?xml version=\"1.0\" encoding=\"SHIFT_JIS\"?>{}", xml("")),
        format!(" <?xml version=\"1.0\"?>{}", xml("")),
    ] {
        assert!(document(&text, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn parser_depth_attributes_definitions_and_native_byte_ceilings_are_separate() {
    let deep = xml(&format!("{}{}", "<N>".repeat(128), "</N>".repeat(128)));
    assert_eq!(
        document(&deep, &mut budget(), &|| false).err(),
        Some(InspectError::StructureLimit)
    );
    let attributes = (0..65).map(|i| format!(" A{i}=\"x\"")).collect::<String>();
    assert_eq!(
        document(
            &xml(&format!("<Statements{attributes}/>")),
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::StructureLimit)
    );
    let definitions = format!(
        "<DataMacros xmlns=\"{NOVEMBER}\">{}</DataMacros>",
        "<DataMacro Event=\"AfterInsert\"/>".repeat(1025)
    );
    assert_eq!(
        document(&definitions, &mut budget(), &|| false).err(),
        Some(InspectError::StructureLimit)
    );
    let large = " ".repeat(2 * 1024 * 1024 + 1);
    assert_eq!(
        document(&large, &mut budget(), &|| false).err(),
        Some(InspectError::StructureLimit)
    );
}

#[test]
fn shared_budgets_and_final_cancellation_publish_no_document() {
    let text = xml("<Statements><Action Name=\"Opaque\"/></Statements>");
    for mut limited in [
        ResourceBudget::new(0, 0, 10_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 0, 1),
    ] {
        assert!(matches!(
            document(&text, &mut limited, &|| false),
            Err(InspectError::Limit(_))
        ));
    }
    let calls = Cell::new(0);
    document(&text, &mut budget(), &|| {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    let current = Cell::new(0);
    assert_eq!(
        document(&text, &mut budget(), &|| {
            current.set(current.get() + 1);
            current.get() == total
        })
        .err(),
        Some(InspectError::Cancelled)
    );
    let mut paid = budget();
    document(&text, &mut paid, &|| false).unwrap();
    assert!(paid.remaining(Resource::Bytes) < budget().remaining(Resource::Bytes));
    assert!(paid.remaining(Resource::Steps) < budget().remaining(Resource::Steps));
}

#[test]
fn namespace_inventory_and_uri_comparison_work_are_bounded() {
    let declarations = |prefix: &str| {
        (0..64)
            .map(|index| format!(" xmlns:{prefix}{index}=\"urn:opaque\""))
            .collect::<String>()
    };
    let body = format!("<N{}><N{}/></N>", declarations("a"), declarations("b"));
    assert_eq!(
        document(&xml(&body), &mut budget(), &|| false).err(),
        Some(InspectError::StructureLimit)
    );

    let uri = format!("urn:{}", "x".repeat(5000));
    let attributes = (0..32)
        .map(|index| format!(" x:A{index}=\"opaque\""))
        .collect::<String>();
    let text = xml(&format!("<N xmlns:x=\"{uri}\"{attributes}/>"));
    document(&text, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        document(
            &text,
            &mut ResourceBudget::new(64 * 1024 * 1024, 0, 100_000),
            &|| false,
        ),
        Err(InspectError::Limit(_))
    ));
}
