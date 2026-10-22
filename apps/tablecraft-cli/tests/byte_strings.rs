mod support;

use support::run;

#[test]
fn utf16_byte_observations_are_not_utf8_or_ansi_counts() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Len('é中🦀') AS Units,LenB('é中🦀') AS Bytes,AscB('中') AS FirstByte,VarType(LenB('x')) AS L,VarType(AscB('x')) AS A,LenB(Null) AS N",
            "--character-code-page",
            "1252",
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
                vec!["Units", "Bytes", "FirstByte", "L", "A", "N"],
                vec!["4", "8", "45", "3", "2", "NULL"]
            ]
        );

        for expression in [
            "AscB('')",
            "AscB(Null)",
            "LenB(1)",
            "LenB(*)",
            "AscB(,)",
            "LenB('x',1/0)",
            "LenB('x'),1/0",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn raw_byte_strings_are_lossless_inside_calls_and_strict_at_stdout() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT LenB(ChrB(255)) AS L,AscB(RightB('🦀',3)) AS A,LenB(CVar(MidB('🦀',2,2))) AS M,VarType(ChrB(0)) AS V,IsArray(ChrB(0)) AS B",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            "L\tA\tM\tV\tB\n1\t216\t2\t8\tFALSE\n"
        );
        for expression in [
            "ChrB(65)",
            "LeftB('A',1)",
            "RightB('🦀',2)",
            "LeftB(Null,1/0)",
            "MidB('A',0)",
            "ChrB(256)",
            "ChrB(-1)",
            "LeftB('A',1,1/0)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}

#[test]
fn binary_instrb_finds_byte_positions_without_publishing_raw_strings() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT InStrB(1,'A中🦀','🦀',0) AS U,InStrB(1,'A',ChrB(0),0) AS Z,InStrB(1,RightB('🦀',3),ChrB(216),0) AS R",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            "U\tZ\tR\n5\t2\t1\n"
        );
        for expression in [
            "InStrB('A','A')",
            "InStrB(1,'A','a',1)",
            "InStrB(Null,'A','A',0)",
            "InStrB(1,Null,'A',1/0)",
            "InStrB(0,'A','A',0)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
