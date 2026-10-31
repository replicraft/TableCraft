//! Literal event registries and complete expected images, before any dispatch change.
use super::super::physical_sources::model as original;
use crate::preservation::table_extra::tests::multievent_sources as programs;
use tablecraft_testkit::access as raw;

const NAMESPACE: &str = "http://schemas.microsoft.com/office/accessservices/2009/11/application";
const SET_GROUP: &str = "<Action Name=\"SetField\"><Argument Name=\"Field\">Group</Argument><Argument Name=\"Value\">[Score]</Argument></Action>";
const SET_SCORE: &str = "<Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[ID]</Argument></Action>";
const DELETE_GUARD: &str = "<ConditionalBlock><If><Condition>[Old].[ID]=301</Condition><Statements><Action Name=\"RaiseError\"><Argument Name=\"Number\">23</Argument><Argument Name=\"Description\">delete rejected</Argument></Action></Statements></If></ConditionalBlock>";
const RAISE: &str = "<Action Name=\"RaiseError\"><Argument Name=\"Number\">17</Argument><Argument Name=\"Description\">rejected</Argument></Action>";

#[derive(Clone, Copy, Debug)]
pub(super) enum Recipe {
    Separate,
    ReversedSelectors,
    SharedProperty,
    DormantDeleteRaise,
    ActiveChangeRaise,
    SequentialChange,
    DormantCurrent,
    DormantSetField,
    UnknownEvent,
    IncompleteSelection,
    DormantOpaque,
    DuplicateSelector,
}

pub(super) const RECIPES: [Recipe; 12] = [
    Recipe::Separate,
    Recipe::ReversedSelectors,
    Recipe::SharedProperty,
    Recipe::DormantDeleteRaise,
    Recipe::ActiveChangeRaise,
    Recipe::SequentialChange,
    Recipe::DormantCurrent,
    Recipe::DormantSetField,
    Recipe::UnknownEvent,
    Recipe::IncompleteSelection,
    Recipe::DormantOpaque,
    Recipe::DuplicateSelector,
];

pub(super) struct Case {
    pub images: original::Case,
    pub properties: Vec<(String, String)>,
    pub selection: Vec<usize>,
    pub events: Vec<Vec<&'static str>>,
    pub changed: bool,
}

fn event(name: &str, body: &str) -> String {
    format!(
        "<DataMacro xmlns=\"{NAMESPACE}\" Event=\"{name}\"><Statements>{body}</Statements></DataMacro>"
    )
}

fn catalog_record(bytes: &[u8], slot: usize) -> Vec<u8> {
    let page = &bytes[4 * 4096..5 * 4096];
    let directory = 14 + slot * 2;
    let start = usize::from(u16::from_le_bytes(
        page[directory..directory + 2].try_into().unwrap(),
    ));
    let end = if slot == 0 {
        4096
    } else {
        usize::from(u16::from_le_bytes(
            page[directory - 2..directory].try_into().unwrap(),
        ))
    };
    assert_eq!(start & 0xe000, 0);
    assert!(start < end && end <= 4096);
    page[start..end].to_vec()
}

pub(super) fn source(code: u8, recipe: Recipe) -> Case {
    let mut images = original::source(code, false, false, false);
    let change = event("BeforeChange", &format!("{SET_GROUP}{SET_SCORE}"));
    let delete = event("BeforeDelete", "");
    let mut properties = vec![("Change".to_owned(), change), ("Delete".to_owned(), delete)];
    let mut events = vec![vec!["BeforeChange"], vec!["BeforeDelete"]];
    let mut selection = vec![0, 1];
    let changed = matches!(
        recipe,
        Recipe::Separate
            | Recipe::ReversedSelectors
            | Recipe::SharedProperty
            | Recipe::DormantDeleteRaise
            | Recipe::SequentialChange
    );

    match recipe {
        Recipe::Separate => {}
        Recipe::ReversedSelectors => selection.reverse(),
        Recipe::SharedProperty => {
            let change = event("BeforeChange", &format!("{SET_GROUP}{SET_SCORE}"));
            let delete = event("BeforeDelete", DELETE_GUARD);
            // Existing envelope parser already admits this DataMacros collection.
            properties = vec![(
                "Combined".to_owned(),
                format!("<DataMacros xmlns=\"{NAMESPACE}\">{change}{delete}</DataMacros>"),
            )];
            events = vec![vec!["BeforeChange", "BeforeDelete"]];
            selection = vec![0];
        }
        Recipe::DormantDeleteRaise => properties[1].1 = event("BeforeDelete", DELETE_GUARD),
        Recipe::ActiveChangeRaise => properties[0].1 = event("BeforeChange", RAISE),
        Recipe::SequentialChange => {
            properties[0].1 = event("BeforeChange", SET_GROUP);
            properties.push(("ChangeLast".to_owned(), event("BeforeChange", SET_SCORE)));
            events.push(vec!["BeforeChange"]);
            selection.push(2);
        }
        Recipe::DormantCurrent => {
            properties[1].1 = event(
                "BeforeDelete",
                "<ConditionalBlock><If><Condition>[ID]=301</Condition><Statements/></If></ConditionalBlock>",
            );
        }
        Recipe::DormantSetField => properties[1].1 = event("BeforeDelete", SET_SCORE),
        Recipe::UnknownEvent => {
            properties[1].1 = event("AfterDelete", "");
            events[1] = vec!["AfterDelete"];
        }
        Recipe::IncompleteSelection => selection = vec![0],
        Recipe::DormantOpaque => {
            properties[1].1 = event(
                "BeforeDelete",
                "<ConditionalBlock><If><Condition>False</Condition><Statements><Action Name=\"FutureAction\"/></Statements></If></ConditionalBlock>",
            );
        }
        Recipe::DuplicateSelector => selection = vec![0, 1, 0],
    }

    let borrowed = properties
        .iter()
        .map(|(name, xml)| (name.as_str(), xml.as_str()))
        .collect::<Vec<_>>();
    let carrier = programs::carrier(code, &borrowed);
    let records = [
        (0, catalog_record(&images.before, 0)),
        (0, catalog_record(&images.before, 1)),
        (0, carrier.record),
    ];
    let catalog = raw::data_page(2, &records);
    for bytes in [&mut images.before, &mut images.expected] {
        bytes[4 * 4096..5 * 4096].copy_from_slice(&catalog);
    }
    if !changed {
        images.expected.clone_from(&images.before);
        images.final_rows.clone_from(&images.original);
    }
    images.extra = carrier.extra;
    images.changed = changed;

    Case {
        images,
        properties,
        selection,
        events,
        changed,
    }
}
