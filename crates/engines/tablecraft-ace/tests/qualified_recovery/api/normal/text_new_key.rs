//! Four exact new-family replacements; existing-member history stays unchanged.
use super::{Storage, budget, scope};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_ace::{
    NativeFittedIntentGroup, NativeFittedUpdates, NativeImage, NativeUpdateIntent,
    SlotScalarUpdate, inspect_table_index_counter, qualify_native_update_recovery, read_table,
    read_table_index,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};

fn rows() -> Vec<Vec<Value>> {
    let values = [
        None,
        Some(""),
        Some("a"),
        Some("A"),
        Some("é"),
        Some("e\u{301}"),
        Some("Straße"),
        Some("Strasse"),
        Some("a  "),
        Some("😀"),
        Some("\0"),
        Some(""),
        Some("カタカナ"),
    ];
    (0..24)
        .map(|ordinal| {
            let position = ordinal * 7 % 13;
            let value = if position == 11 {
                Value::Text("a".repeat(255))
            } else {
                values[position].map_or(Value::Null, |text| Value::Text(text.to_owned()))
            };
            vec![Value::Int32(ordinal as i32 + 1), value]
        })
        .collect()
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
#[ignore = "requires6 independently qualified new-family Text UPDATE source views"]
fn new_general_key_updates_replace_existing_members_without_incrementing_history() {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_TEXT_NEW_KEY_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    assert_eq!(
        hash(&fs::read(directory.join("manifest.json")).unwrap()),
        "3b22d4be6b6730dce7f19da23e103c794818e86380ceb24f8a9d6da09a6cea43"
    );
    let mut images = Vec::new();
    for (id, filename, length, digest) in [
        (
            "h5-original",
            "Encoded-V2016-before.accdb",
            1163264,
            "e2b5193dc4eb6c024e1e2883e894d64b4ff82c435ca441f0745e677615fc9276",
        ),
        (
            "h5-textlegacyleaf-new-family-ss",
            "h5-textlegacyleaf-new-family-ss.accdb",
            1163264,
            "504fdd0ca3f0ec53f933425c3e91565af431def3bdc22a0aeee47caa22081c36",
        ),
        (
            "h5-textmodernleaf-new-family-ss",
            "h5-textmodernleaf-new-family-ss.accdb",
            1163264,
            "ad75b859be057c8bb30efb55378a9e1c7bcf58e5826e6c67b98feab3e8d17912",
        ),
        (
            "h6-original",
            "Encoded-V2019-before.accdb",
            1236992,
            "636e137ee3a1d51f6525072de87963cb33a03b01a253be1d2124cf6bb4746279",
        ),
        (
            "h6-textlegacyleaf-new-family-ss",
            "h6-textlegacyleaf-new-family-ss.accdb",
            1236992,
            "4e25b090c33a4a66edf70261c2afc95204e326fb3de4a69291258033ceaaa7dc",
        ),
        (
            "h6-textmodernleaf-new-family-ss",
            "h6-textmodernleaf-new-family-ss.accdb",
            1236992,
            "f89c230bcac0d0113949fc7cc8e2a9d440b4504ee85378f99633b93a0aacbc20",
        ),
    ] {
        let raw = fs::read(directory.join(filename)).unwrap();
        assert_eq!(raw.len(), length);
        assert_eq!(hash(&raw), digest);
        images.push((id, raw));
    }
    let mut contexts = 0;
    for (header, table, identity, text_history) in [
        (5, "TextLegacyLeaf", "h5-textlegacyleaf-new-family-ss", 7),
        (5, "TextModernLeaf", "h5-textmodernleaf-new-family-ss", 8),
        (6, "TextLegacyLeaf", "h6-textlegacyleaf-new-family-ss", 7),
        (6, "TextModernLeaf", "h6-textmodernleaf-new-family-ss", 8),
    ] {
        let original = format!("h{header}-original");
        let before = &images.iter().find(|(id, _)| *id == original).unwrap().1;
        let after = &images.iter().find(|(id, _)| *id == identity).unwrap().1;
        let image =
            NativeImage::capture(&mut Cursor::new(before), scope(), &mut budget(), &|| false)
                .unwrap();
        let updates = vec![SlotScalarUpdate::new(
            1,
            Value::Text("Strasse".into()),
            Value::Text("ss".into()),
        )];
        let group = NativeFittedIntentGroup::new(
            "KeyValue",
            NativeFittedUpdates::Variable(updates),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let plan = image
            .plan_recoverable_ordinary_fitted_updates(table, vec![group], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), after);
        assert_eq!(plan.changed_rows(), 1);
        let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
        assert_eq!(&wire[..8], b"TCNI0005");
        let mut target = Storage::new(before.to_vec());
        let mut log = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare(&mut target, &mut log, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(target.bytes.get_ref(), after);
        let mut expected = rows();
        expected[1][1] = Value::Text("ss".into());
        let observed = read_table(
            &mut Cursor::new(after),
            scope(),
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(observed.rows(), expected);
        for (name, history) in [
            ("Primary", 24),
            ("Asc", text_history),
            ("Twin", text_history),
            ("Desc", text_history),
        ] {
            for raw in [before, after] {
                let actual = inspect_table_index_counter(
                    &mut Cursor::new(raw),
                    scope(),
                    table,
                    name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(actual.value(), history);
            }
        }
        let asc = read_table_index(
            &mut Cursor::new(after),
            scope(),
            table,
            "Asc",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let twin = read_table_index(
            &mut Cursor::new(after),
            scope(),
            table,
            "Twin",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(asc.rows(), twin.rows());
        let writes = (target.writes, log.writes);
        let intent = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
        let qualified =
            qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false)
                .unwrap();
        assert_eq!(
            qualified
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap(),
            RecoveryOutcome::Committed
        );
        assert_eq!((target.writes, log.writes), writes);
        contexts += 1;
    }
    assert_eq!(contexts, 4);
}
