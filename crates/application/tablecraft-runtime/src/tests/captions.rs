use super::{query, request, stored::opened};
use crate::{Command, Response};
use tablecraft_testkit::fixtures::access::field_presentation;

#[test]
fn native_caption_does_not_replace_field_identity_or_query_aliases() {
    for (native_type, caption) in [
        (10, "Visible score"),
        (10, "Id"),
        (10, ""),
        (12, "Visible score"),
        (12, "Id"),
        (12, ""),
    ] {
        let bytes = field_presentation::single("Caption", 1, native_type, caption, false);
        let (_directory, path, worker, _revision) = opened(&bytes);
        let Response::Data { columns, rows, .. } = request(
            &worker,
            Command::Page {
                table: "Items".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected native page");
        };
        assert_eq!(columns[0].name, "Id");
        assert_eq!(columns[0].caption, None);
        assert_eq!(columns[1].name, "Score");
        assert_eq!(columns[1].caption.as_deref(), Some(caption));
        assert_eq!(rows.len(), 3);

        let Response::Data { columns, .. } =
            request(&worker, query("SELECT Score AS [Exact alias] FROM Items")).unwrap()
        else {
            panic!("expected query data");
        };
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].name, "Exact alias");
        assert_eq!(columns[0].caption, None);
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}
