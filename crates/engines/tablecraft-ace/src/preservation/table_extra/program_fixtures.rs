//! Self-authored XML and literal expected source spans; no production imports.
#[derive(Clone, Copy)]
pub(super) enum Outcome {
    RecognizedDesktopSubset,
    ContainsOpaqueStructure,
    Malformed,
    Unsupported,
}

pub(super) struct NodeGolden {
    pub kind: &'static str,
    pub parent: Option<usize>,
    pub span: (usize, usize),
    pub content: Option<(usize, usize)>,
    pub number: Option<(usize, usize)>,
    pub description: Option<(usize, usize)>,
    pub diagnostic: Option<&'static str>,
    pub definition: usize,
}

pub(super) struct SourceGolden {
    pub name: &'static str,
    pub xml: &'static str,
    pub outcome: Outcome,
    pub definitions: usize,
    pub nodes: &'static [NodeGolden],
}

pub(super) fn sources() -> &'static [SourceGolden] {
    &[
        SourceGolden {
            name: "april_simple",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/04/application" Event="BeforeChange">
  <Statements>
    <Action Name="RaiseError">
      <Argument Name="Number">23</Argument>
      <Argument Name="Description">local guard</Argument>
    </Action>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::RecognizedDesktopSubset,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 289),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "RaiseError",
                    parent: Some(0),
                    span: (131, 273),
                    content: None,
                    number: Some((188, 190)),
                    description: Some((237, 248)),
                    diagnostic: None,
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "present_empty_arguments",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <Statements>
    <Action Name="RaiseError">
      <Argument Name="Number"/>
      <Argument Name="Description"></Argument>
    </Action>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::RecognizedDesktopSubset,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 266),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "RaiseError",
                    parent: Some(0),
                    span: (131, 250),
                    content: None,
                    number: Some((189, 189)),
                    description: Some((225, 225)),
                    diagnostic: None,
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "bom_alias_unicode_cdata",
            xml: concat!(
                "\u{feff}",
                r##"<?xml version="1.0" encoding="UTF-16"?>
<m:DataMacro xmlns:m="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeDelete">
  <m:Statements>
    <m:ConditionalBlock>
      <m:If>
        <m:Condition><![CDATA[[Étage] > 0]]></m:Condition>
        <m:Statements>
          <m:Action Name="RaiseError">
            <m:Argument Name="Number">[Old].[Key]</m:Argument>
          </m:Action>
        </m:Statements>
      </m:If>
    </m:ConditionalBlock>
  </m:Statements>
</m:DataMacro>"##
            ),
            outcome: Outcome::RecognizedDesktopSubset,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (161, 502),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "ConditionalBlock",
                    parent: Some(0),
                    span: (180, 484),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "If",
                    parent: Some(1),
                    span: (207, 458),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Condition",
                    parent: Some(2),
                    span: (222, 273),
                    content: Some((235, 259)),
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Statements",
                    parent: Some(2),
                    span: (282, 444),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "RaiseError",
                    parent: Some(4),
                    span: (307, 420),
                    content: None,
                    number: Some((374, 385)),
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "present_empty_condition",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <Statements>
    <ConditionalBlock>
      <If>
        <Condition/>
        <Statements/>
      </If>
    </ConditionalBlock>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::RecognizedDesktopSubset,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 255),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "ConditionalBlock",
                    parent: Some(0),
                    span: (131, 239),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "If",
                    parent: Some(1),
                    span: (156, 215),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Condition",
                    parent: Some(2),
                    span: (169, 181),
                    content: Some((181, 181)),
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Statements",
                    parent: Some(2),
                    span: (190, 203),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "number_expression_not_typed",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <Statements>
    <Action Name="RaiseError">
      <Argument Name="Number">not a numeric proof</Argument>
    </Action>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::RecognizedDesktopSubset,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 248),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "RaiseError",
                    parent: Some(0),
                    span: (131, 232),
                    content: None,
                    number: Some((188, 207)),
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "named_and_unknown_event_empty_bodies",
            xml: r##"<DataMacros xmlns="http://schemas.microsoft.com/office/accessservices/2009/04/application">
  <DataMacro Name="NamedCheck">
    <Statements/>
  </DataMacro>
  <DataMacro Event="FutureEvent">
    <Statements/>
  </DataMacro>
</DataMacros>"##,
            outcome: Outcome::RecognizedDesktopSubset,
            definitions: 2,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (128, 141),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (195, 208),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 1,
                },
            ],
        },
        SourceGolden {
            name: "unknown_action",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <Statements>
    <Action Name="FutureAction"/>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::ContainsOpaqueStructure,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 176),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Opaque",
                    parent: Some(0),
                    span: (131, 160),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: Some("UnknownAction"),
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "setfield_context_unqualified",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <Statements>
    <Action Name="SetField">
      <Argument Name="Field">Status</Argument>
      <Argument Name="Value">Null</Argument>
    </Action>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::ContainsOpaqueStructure,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 277),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Opaque",
                    parent: Some(0),
                    span: (131, 261),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: Some("UnsupportedAction"),
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "missing_number_element",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <Statements>
    <Action Name="RaiseError">
      <Argument Name="Description">opaque desktop profile</Argument>
    </Action>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::ContainsOpaqueStructure,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 256),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Opaque",
                    parent: Some(0),
                    span: (131, 240),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: Some("MissingArgument"),
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "duplicate_number_elements",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <Statements>
    <Action Name="RaiseError">
      <Argument Name="Number">1</Argument>
      <Argument Name="Number">2</Argument>
    </Action>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::ContainsOpaqueStructure,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 273),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Opaque",
                    parent: Some(0),
                    span: (131, 257),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: Some("DuplicateArgument"),
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "unknown_argument",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <Statements>
    <Action Name="RaiseError">
      <Argument Name="Number">1</Argument>
      <Argument Name="Future">preserve this too</Argument>
    </Action>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::ContainsOpaqueStructure,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 289),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Opaque",
                    parent: Some(0),
                    span: (131, 273),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: Some("UnknownArgument"),
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "foreign_argument_element",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <Statements>
    <Action Name="RaiseError" xmlns:x="urn:future">
      <x:Argument Name="Number">1</x:Argument>
    </Action>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::ContainsOpaqueStructure,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 255),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Opaque",
                    parent: Some(0),
                    span: (131, 239),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: Some("ForeignElement"),
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "foreign_attribute_same_local_name",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <Statements>
    <Action Name="RaiseError" xmlns:x="urn:future" x:Name="opaque">
      <Argument Name="Number">1</Argument>
    </Action>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::ContainsOpaqueStructure,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 267),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Opaque",
                    parent: Some(0),
                    span: (131, 251),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: Some("ForeignAttribute"),
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "foreign_known_local_name",
            xml: r##"<m:DataMacro xmlns:m="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <x:Statements xmlns:x="urn:foreign"><x:Action Name="RaiseError"/></x:Statements>
</m:DataMacro>"##,
            outcome: Outcome::ContainsOpaqueStructure,
            definitions: 1,
            nodes: &[NodeGolden {
                kind: "Opaque",
                parent: None,
                span: (118, 198),
                content: None,
                number: None,
                description: None,
                diagnostic: Some("ForeignElement"),
                definition: 0,
            }],
        },
        SourceGolden {
            name: "prefix_rebinding",
            xml: r##"<m:DataMacro xmlns:m="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <m:Statements xmlns:m="urn:foreign"><m:Action Name="RaiseError"/></m:Statements>
</m:DataMacro>"##,
            outcome: Outcome::ContainsOpaqueStructure,
            definitions: 1,
            nodes: &[NodeGolden {
                kind: "Opaque",
                parent: None,
                span: (118, 198),
                content: None,
                number: None,
                description: None,
                diagnostic: Some("ForeignElement"),
                definition: 0,
            }],
        },
        SourceGolden {
            name: "missing_condition",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <Statements>
    <ConditionalBlock><If><Statements/></If></ConditionalBlock>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::ContainsOpaqueStructure,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 206),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Opaque",
                    parent: Some(0),
                    span: (131, 190),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: Some("UnsupportedStructure"),
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "duplicate_condition",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <Statements>
    <ConditionalBlock><If><Condition/><Condition/><Statements/></If></ConditionalBlock>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::ContainsOpaqueStructure,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 230),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Opaque",
                    parent: Some(0),
                    span: (131, 214),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: Some("UnsupportedStructure"),
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "else_outside_initial_subset",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
  <Statements>
    <ConditionalBlock><If><Condition/><Statements/></If><Else><Statements/></Else></ConditionalBlock>
  </Statements>
</DataMacro>"##,
            outcome: Outcome::ContainsOpaqueStructure,
            definitions: 1,
            nodes: &[
                NodeGolden {
                    kind: "Statements",
                    parent: None,
                    span: (114, 244),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: None,
                    definition: 0,
                },
                NodeGolden {
                    kind: "Opaque",
                    parent: Some(0),
                    span: (131, 228),
                    content: None,
                    number: None,
                    description: None,
                    diagnostic: Some("UnsupportedStructure"),
                    definition: 0,
                },
            ],
        },
        SourceGolden {
            name: "missing_statements",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">

</DataMacro>"##,
            outcome: Outcome::ContainsOpaqueStructure,
            definitions: 1,
            nodes: &[NodeGolden {
                kind: "Opaque",
                parent: None,
                span: (0, 125),
                content: None,
                number: None,
                description: None,
                diagnostic: Some("UnsupportedStructure"),
                definition: 0,
            }],
        },
        SourceGolden {
            name: "xml_unclosed",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
<Statements><Action Name="RaiseError">
</DataMacro>"##,
            outcome: Outcome::Malformed,
            definitions: 0,
            nodes: &[],
        },
        SourceGolden {
            name: "xml_duplicate_expanded_attribute",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
<Statements xmlns:a="urn:x" xmlns:b="urn:x" a:V="1" b:V="2"/>
</DataMacro>"##,
            outcome: Outcome::Malformed,
            definitions: 0,
            nodes: &[],
        },
        SourceGolden {
            name: "xml_unknown_entity",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
<Statements>&never;</Statements>
</DataMacro>"##,
            outcome: Outcome::Unsupported,
            definitions: 0,
            nodes: &[],
        },
        SourceGolden {
            name: "xml_external_dtd",
            xml: r##"<!DOCTYPE DataMacro SYSTEM "file:///never-open"><DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
<Statements/>
</DataMacro>"##,
            outcome: Outcome::Unsupported,
            definitions: 0,
            nodes: &[],
        },
        SourceGolden {
            name: "xml_unbound_prefix",
            xml: r##"<DataMacro xmlns="http://schemas.microsoft.com/office/accessservices/2009/11/application" Event="BeforeChange">
<u:Statements/>
</DataMacro>"##,
            outcome: Outcome::Unsupported,
            definitions: 0,
            nodes: &[],
        },
        SourceGolden {
            name: "xml_unknown_root_namespace",
            xml: r##"<DataMacro xmlns="urn:unknown" Event="BeforeChange">
<Statements/>
</DataMacro>"##,
            outcome: Outcome::Unsupported,
            definitions: 0,
            nodes: &[],
        },
    ]
}
