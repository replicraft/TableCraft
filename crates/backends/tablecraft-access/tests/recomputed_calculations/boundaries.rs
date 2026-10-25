use super::*;
use std::cell::RefCell;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::Resource;

fn watched(expression: &str, forbidden: Rc<Cell<bool>>) -> NativeSource<'static, Watching> {
    NativeSource::new(
        Watching {
            bytes: Cursor::new(calculated_fixture::database_with_expression(
                &[],
                expression,
            )),
            forbidden,
            reads: Rc::new(Cell::new(0)),
        },
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn invalid_selected_declarations_and_aliases_refuse_without_reading_even_empty_inputs() {
    for expression in [
        "Now()",
        "[Score]",
        "[Missing]+1",
        "IIf(True,1,Now())",
        "[Items].[Id]+1",
    ] {
        let source = watched(expression, Rc::new(Cell::new(true)));
        assert!(
            RecomputedNativeSource::new(
                &source,
                &[CalculationView::new("Fresh", "Items", &["Score"])],
                ParseLimits::default(),
            )
            .is_err()
        );
    }

    let source = watched("[Id]*2", Rc::new(Cell::new(true)));
    for requests in [
        vec![CalculationView::new("ITEMS", "Items", &["Id"])],
        vec![CalculationView::new("Fresh", "Items", &["Id", "ID"])],
        vec![CalculationView::new("Fresh", "Items", &[])],
        vec![CalculationView::new("Fresh", "Missing", &["Id"])],
        vec![CalculationView::new("", "Items", &["Id"])],
        vec![CalculationView::new("bad\nname", "Items", &["Id"])],
        vec![CalculationView::new("bad]name", "Items", &["Id"])],
        vec![
            CalculationView::new("Fresh", "Items", &["Id"]),
            CalculationView::new("FRESH", "Items", &["Score"]),
        ],
    ] {
        assert!(RecomputedNativeSource::new(&source, &requests, ParseLimits::default()).is_err());
    }
}

#[test]
fn constructor_and_lookup_use_the_original_cumulative_pool_without_refunds() {
    let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let usage = {
        let pool = RefCell::new(budget());
        let source = NativeSource::new_with_shared_budget(
            Cursor::new(calculated_fixture::database(&[])),
            CatalogScope::new(1),
            &pool,
            &|| false,
        )
        .unwrap();
        let before = resources.map(|resource| pool.borrow().remaining(resource));
        let views = RecomputedNativeSource::new(
            &source,
            &[CalculationView::new("Fresh", "Items", &["Id", "Score"])],
            ParseLimits::default(),
        )
        .unwrap();
        let usage = resources.map(|resource| {
            let index = resources
                .iter()
                .position(|value| *value == resource)
                .unwrap();
            before[index] - pool.borrow().remaining(resource)
        });
        assert!(views.query("Fresh").unwrap().is_some());
        let remaining = pool.borrow().remaining(Resource::Steps);
        assert!(views.query("Fresh").unwrap().is_some());
        assert!(pool.borrow().remaining(Resource::Steps) < remaining);
        usage
    };

    for (resource, used) in resources.into_iter().zip(usage) {
        if used == 0 {
            continue;
        }
        let pool = RefCell::new(budget());
        let source = NativeSource::new_with_shared_budget(
            Cursor::new(calculated_fixture::database(&[])),
            CatalogScope::new(1),
            &pool,
            &|| false,
        )
        .unwrap();
        let remaining = pool.borrow().remaining(resource);
        pool.borrow_mut()
            .charge(resource, remaining - used + 1)
            .unwrap();
        assert!(
            RecomputedNativeSource::new(
                &source,
                &[CalculationView::new("Fresh", "Items", &["Id", "Score"])],
                ParseLimits::default(),
            )
            .is_err()
        );
        assert!(pool.borrow().remaining(resource) < used);
    }
}

#[test]
fn every_constructor_cancellation_checkpoint_refuses_before_user_rows() {
    let checkpoints = Cell::new(0_usize);
    let stop = Cell::new(None);
    let cancelled = || {
        let checkpoint = checkpoints.get();
        checkpoints.set(checkpoint + 1);
        stop.get().is_some_and(|stop| checkpoint >= stop)
    };

    let total = {
        let source = NativeSource::new(
            Cursor::new(calculated_fixture::database(&[])),
            CatalogScope::new(1),
            budget(),
            &cancelled,
        )
        .unwrap();
        checkpoints.set(0);
        let _views = RecomputedNativeSource::new(
            &source,
            &[CalculationView::new("Fresh", "Items", &["Id", "Score"])],
            ParseLimits::default(),
        )
        .unwrap();
        checkpoints.get()
    };
    assert!(total > 0);

    for checkpoint in 0..total {
        stop.set(None);
        let source = NativeSource::new(
            Watching {
                bytes: Cursor::new(calculated_fixture::database(&[])),
                forbidden: Rc::new(Cell::new(true)),
                reads: Rc::new(Cell::new(0)),
            },
            CatalogScope::new(1),
            budget(),
            &cancelled,
        )
        .unwrap();
        checkpoints.set(0);
        stop.set(Some(checkpoint));
        assert!(matches!(
            RecomputedNativeSource::new(
                &source,
                &[CalculationView::new("Fresh", "Items", &["Id", "Score"])],
                ParseLimits::default(),
            ),
            Err(CalculationError::Query(QueryError::Cancelled))
                | Err(CalculationError::Native(InspectError::Cancelled))
        ));
    }
}
