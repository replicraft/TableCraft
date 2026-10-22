mod support;

use support::run;

fn row(bytes: &[u8]) -> Vec<f32> {
    csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(bytes)
        .records()
        .next()
        .unwrap()
        .unwrap()
        .iter()
        .map(|text| text.parse().unwrap())
        .collect()
}

#[test]
fn rnd_state_is_shared_within_a_command_and_seed_selection_is_explicit() {
    for mode in ["eval", "run"] {
        let result = run(&["query", mode, "SELECT Rnd(),Rnd(0),Rnd(-1),Rnd(0),Rnd()"]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let values = row(&result.stdout);
        assert_eq!(values[0], 0.7055475);
        assert_eq!(values[0], values[1]);
        assert_eq!(values[2], 0.22400701);
        assert_eq!(values[2], values[3]);
        assert_ne!(values[3], values[4]);
        let again = run(&["query", mode, "SELECT Rnd(),Rnd(0),Rnd(-1),Rnd(0),Rnd()"]);
        assert_eq!(again.stdout, result.stdout);
        let seeded = run(&["query", mode, "SELECT Rnd(0)", "--random-seed", "0"]);
        assert!(seeded.status.success());
        assert_eq!(row(&seeded.stdout), [0.0]);
        for seed in ["16777216", "-1", "not-a-number"] {
            let result = run(&["query", mode, "SELECT Rnd()", "--random-seed", seed]);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
        for expression in ["Rnd(Null)", "Rnd(1/0)", "Rnd(1/0,1)", "Rnd(*)"] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
