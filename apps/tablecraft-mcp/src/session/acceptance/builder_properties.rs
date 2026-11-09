use crate::session::tests::{acknowledge, opened, save};
use serde_json::json;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

#[test]
fn manual_design_no_key_confirmation_persists_collision_safe_primary_and_explicit_text_properties()
{
    let before = canonical_blank_creation::source();
    let (_directory, path, mut session) = opened(&before);
    let revision = session.revision.unwrap();
    let request = json!({
        "document_revision": revision,
        "table": "Designed",
        "initial_autonumber": false,
        "add_incrementing_primary_key": true,
        "fields": [
            {
                "name": "ID",
                "kind": "number"
            },
            {
                "name": "Name",
                "kind": "short_text",
                "max_utf16_units": 32,
                "required": false,
                "allow_zero_length": true,
                "default_mode": "absent",
                "description": "Human-readable name"
            }
        ]
    });
    crate::requests::validate("review_table_creation", &request).unwrap();
    let review = session.execute("review_table_creation", request).unwrap();
    assert_eq!(review["columns"], 3);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let id = save(&mut session, "apply_table_creation", review);
    acknowledge(&mut session, id, &path);
    let revision = session.revision.unwrap();
    let metadata = session
        .execute(
            "describe_table",
            json!({
                "document_revision": revision,
                "table": "Designed"
            }),
        )
        .unwrap();
    let fields = metadata["fields"].as_array().unwrap();
    assert_eq!(fields[0]["name"], "ID1");
    assert_eq!(fields[1]["name"], "ID");
    let name = fields.iter().find(|field| field["name"] == "Name").unwrap();
    assert_eq!(name["storage_bytes"], 64);
    let properties = name["properties"].as_array().unwrap();
    let required = properties
        .iter()
        .find(|property| property["name"] == "Required")
        .unwrap();
    assert_eq!(
        required["observation"],
        json!({
            "kind": "boolean",
            "value": false
        })
    );
    let allow = properties
        .iter()
        .find(|property| property["name"] == "AllowZeroLength")
        .unwrap();
    assert_eq!(
        allow["observation"],
        json!({
            "kind": "boolean",
            "value": true
        })
    );
    let default = properties
        .iter()
        .find(|property| property["name"] == "DefaultValue")
        .unwrap();
    assert_eq!(default["observation"]["kind"], "Absent");
    let description = properties
        .iter()
        .find(|property| property["name"] == "Description")
        .unwrap();
    assert_eq!(
        description["observation"],
        json!({
            "kind": "text",
            "value": "Human-readable name"
        })
    );
    let insertion = session
        .execute(
            "begin_insert",
            json!({
                "document_revision": revision,
                "table": "Designed"
            }),
        )
        .unwrap();
    assert_eq!(insertion["fields"][0]["name"], "ID1");
    assert_eq!(insertion["fields"][0]["auto_number"], true);
    assert_eq!(insertion["fields"][1]["auto_number"], false);
}
