pub(in super::super) struct TextCase {
    pub name: &'static str,
    pub raw: &'static str,
    pub decoded: &'static str,
}

pub(in super::super) const CASES: &[TextCase] = &[
    TextCase {
        name: "plain",
        raw: "[Score] > 3",
        decoded: "[Score] > 3",
    },
    TextCase {
        name: "predefined comparison",
        raw: "[Score]&lt;3 &amp; [Other]&gt;2",
        decoded: "[Score]<3 & [Other]>2",
    },
    TextCase {
        name: "predefined quotes",
        raw: "&quot;a&quot; &apos;b&apos;",
        decoded: "\"a\" 'b'",
    },
    TextCase {
        name: "numeric references",
        raw: "&#65;&#x42;&#x1F642;",
        decoded: "AB🙂",
    },
    TextCase {
        name: "CDATA operators",
        raw: "<![CDATA[[Score]<3 & \"x\"]]>",
        decoded: "[Score]<3 & \"x\"",
    },
    TextCase {
        name: "adjacent content",
        raw: "a<![CDATA[&lt;]]>&amp;b",
        decoded: "a&lt;&b",
    },
    TextCase {
        name: "comment",
        raw: "a<!-- not expression text -->b",
        decoded: "ab",
    },
    TextCase {
        name: "literal line endings",
        raw: " a\r\nb\rc\nd\t ",
        decoded: " a\nb\nc\nd\t ",
    },
    TextCase {
        name: "CDATA line endings",
        raw: "<![CDATA[a\r\nb\rc]]>",
        decoded: "a\nb\nc",
    },
    TextCase {
        name: "referenced carriage return",
        raw: "a&#13;b&#xD;c",
        decoded: "a\rb\rc",
    },
    TextCase {
        name: "Unicode",
        raw: "Étage &amp; 漢字",
        decoded: "Étage & 漢字",
    },
    TextCase {
        name: "interior BOM",
        raw: "\u{feff}value",
        decoded: "\u{feff}value",
    },
    TextCase {
        name: "empty",
        raw: "",
        decoded: "",
    },
    TextCase {
        name: "expression spelling",
        raw: "=\"guard \" &amp; [Old].[Id]",
        decoded: "=\"guard \" & [Old].[Id]",
    },
    TextCase {
        name: "literal description",
        raw: "local guard text",
        decoded: "local guard text",
    },
    TextCase {
        name: "decode once",
        raw: "&#38;lt;",
        decoded: "&lt;",
    },
];

pub(in super::super) fn document(raw: &str, bom: bool) -> String {
    let prefix = if bom { "\u{feff}" } else { "" };
    format!(
        "{prefix}<DataMacro xmlns=\"http://schemas.microsoft.com/office/accessservices/2009/11/application\" Event=\"BeforeChange\"><Statements><ConditionalBlock><If><Condition>{raw}</Condition><Statements><Action Name=\"RaiseError\"><Argument Name=\"Number\">{raw}</Argument><Argument Name=\"Description\">{raw}</Argument></Action></Statements></If></ConditionalBlock></Statements></DataMacro>"
    )
}
