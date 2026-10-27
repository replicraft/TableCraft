use super::*;
use tablecraft_core::values::{Currency, Decimal};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1024 * 1024, 1000, 10000)
}

fn edge(parent: usize, child: usize) -> Link {
    Link {
        parent,
        child,
        matches: Vec::new(),
    }
}

#[test]
fn directed_diamond_and_parallel_edges_pass_without_recursive_traversal() {
    let links = [edge(0, 1), edge(0, 2), edge(1, 3), edge(2, 3), edge(0, 1)];
    acyclic(4, &links, &mut budget(), &|| false).unwrap();
}

#[test]
fn self_and_longer_cycles_refuse_even_when_no_original_rows_match() {
    for links in [vec![edge(0, 0)], vec![edge(0, 1), edge(1, 2), edge(2, 0)]] {
        assert!(matches!(
            acyclic(3, &links, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        ));
    }
}

#[test]
fn exact_atoms_preserve_scaled_extrema_and_nullable_component_positions() {
    let values = [
        Value::Currency(Currency::from_scaled(i64::MIN)),
        Value::Null,
        Value::Decimal(Decimal::new(-Decimal::MAX_COEFFICIENT, 4).unwrap()),
        Value::Int64(i64::MAX),
    ];
    let result = key(&values, &[3, 1, 0, 2]).unwrap();
    assert!(result[0] == Some(Atom::Integral(i64::MAX)));
    assert!(result[1].is_none());
    assert!(result[2] == Some(Atom::Integral(i64::MIN)));
    assert!(result[3] == Some(Atom::Decimal(-Decimal::MAX_COEFFICIENT)));
    assert!(result[4..].iter().all(Option::is_none));
}
