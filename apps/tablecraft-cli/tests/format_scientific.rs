mod support;

use support::run;

#[test]
fn scientific_scaling_sections_and_profiles_execute_in_both_commands() {
    for mode in ["eval", "run"] {
        for (profile, expected) in [
            (
                "en-US",
                ["\"1.23E+04\"", "\"123.46\"", "\"100\"", "\"1.23e004\""],
            ),
            (
                "de-DE",
                ["\"1,23E+04\"", "\"123,46\"", "\"100\"", "\"1,23e004\""],
            ),
        ] {
            let result = run(&[
                "query",
                mode,
                "SELECT Format(12345,'Scientific'),Format(123456,'0,.00'),Format(100000000,'##0,,'),Format(12345,'0.00e-000')",
                "--number-format",
                profile,
            ]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let row = csv::ReaderBuilder::new()
                .delimiter(b'\t')
                .from_reader(result.stdout.as_slice())
                .records()
                .next()
                .unwrap()
                .unwrap();
            assert_eq!(row.iter().collect::<Vec<_>>(), expected);
        }
        for expression in [
            "Format(1,'0E+')",
            "Format(1,'0.00,,')",
            "Format(1,'0E+00E+0')",
            "Format(Null,'0E+00',1/0)",
        ] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT 1,{expression}"),
                "--number-format",
                "en-US",
            ]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
