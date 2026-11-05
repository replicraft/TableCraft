use super::*;
use crate::{
    controls::{SCOPE, budget},
    source::Source,
};
use bytes::Bytes;
use tablecraft_ace::inspect_table_properties;
use tablecraft_testkit::fixtures::access::{field_presentation, wide_properties};

fn properties(key: &str, text: &str, flags: u8, native_type: u8, scope: u16) -> TableProperties {
    let blocks = [
        wide_properties::names(&[key]),
        field_presentation::score_group(
            scope,
            &[field_presentation::property(
                0,
                flags,
                native_type,
                &wide_properties::text(text),
            )],
        ),
    ];
    let bytes = field_presentation::database(&blocks, true);
    let mut work = budget();
    let (mut source, _) = Source::open(Bytes::from(bytes), "", &mut work, &|| false).unwrap();
    inspect_table_properties(&mut source, SCOPE, "Items", &mut work, &|| false).unwrap()
}

#[test]
fn captions_preserve_exact_identity_empty_values_and_column_scope() {
    for label in ["Visible score", "Id", "", "Größe"] {
        let properties = properties("Caption", label, 1, 10, 1);
        let mut output = Presentation::new(&|| false);
        let column = native("sCoRe", Some(&properties), &mut output).unwrap();
        assert_eq!(column.name, "sCoRe");
        assert_eq!(column.caption.as_deref(), Some(label));
        assert_eq!(
            native("Id", Some(&properties), &mut output)
                .unwrap()
                .caption,
            None
        );
    }
}

#[test]
fn absent_unqualified_and_non_column_captions_do_not_fabricate_labels() {
    for (key, flags, native_type, scope) in [
        ("Caption", 0, 10, 1),
        ("Caption", 1, 3, 1),
        ("Caption", 1, 10, 2),
        ("Other", 1, 10, 1),
    ] {
        let properties = properties(key, "Visible score", flags, native_type, scope);
        let mut output = Presentation::new(&|| false);
        assert_eq!(
            native("Score", Some(&properties), &mut output)
                .unwrap()
                .caption,
            None
        );
    }
    assert_eq!(
        native("Score", None, &mut Presentation::new(&|| false))
            .unwrap()
            .caption,
        None
    );
}

#[test]
fn cancelled_caption_publication_is_not_an_absent_caption() {
    let properties = properties("Caption", "Visible score", 1, 10, 1);
    assert!(matches!(
        native("Score", Some(&properties), &mut Presentation::new(&|| true)),
        Err(Error::Cancelled)
    ));
}
