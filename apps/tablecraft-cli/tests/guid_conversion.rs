mod support;

use support::run;

#[test]
fn guidfromstring_returns_byte_array_shape_and_buffers_errors() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT GUIDFromString('6B29FC40-CA47-1067-B31D-00DD010662DA') AS G,VarType(GUIDFromString('6B29FC40-CA47-1067-B31D-00DD010662DA')) AS T,IsArray(GUIDFromString('6B29FC40-CA47-1067-B31D-00DD010662DA')) AS A",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let rows: Vec<Vec<String>> = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_reader(result.stdout.as_slice())
            .records()
            .map(|row| row.unwrap().iter().map(str::to_owned).collect())
            .collect();
        assert_eq!(
            rows,
            [
                vec!["G", "T", "A"],
                vec!["0x40fc296b47ca6710b31d00dd010662da", "8209", "TRUE"]
            ]
        );

        for expression in [
            "GUIDFromString(Null)",
            "GUIDFromString('invalid')",
            "GUIDFromString(1)",
            "GUIDFromString(*)",
            "GUIDFromString(,)",
            "GUIDFromString('invalid',1/0)",
            "VBA.GUIDFromString('invalid')",
            "GUIDFromString('6B29FC40-CA47-1067-B31D-00DD010662DA'),1/0",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn stringfromguid_uses_shared_literal_text_format_and_buffers_refused_calls() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT StringFromGUID(GUIDFromString('00112233-4455-6677-8899-aabbccddeeff')),VarType(StringFromGUID(GUIDFromString('00112233-4455-6677-8899-aabbccddeeff')))",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let rows: Vec<Vec<String>> = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice())
            .records()
            .map(|row| row.unwrap().iter().map(str::to_owned).collect())
            .collect();
        assert_eq!(
            rows,
            [vec![
                "\"{guid {00112233-4455-6677-8899-AABBCCDDEEFF}}\"",
                "8"
            ]]
        );
        for expression in [
            "StringFromGUID(Null)",
            "StringFromGUID(1)",
            "StringFromGUID('not bytes')",
            "StringFromGUID(*)",
            "StringFromGUID()",
            "StringFromGUID(1,1/0)",
            "StringFromGUID(1/0)",
            "VBA.StringFromGUID(1)",
            "StringFromGUID(GUIDFromString('00000000-0000-0000-0000-000000000000')),1/0",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
