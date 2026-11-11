use super::{Data, budget, scalar};
use crate::{
    ExecutionOptions, ParameterDiscoveryMode, ParameterOrigin, PreparationOptions, QueryError,
    Source, parameter_discovery_mode, prepare, prepare_with_parameter_discovery,
};
use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_schema::Column;
use tablecraft_sql::{ParseLimits, parse_query, query::Query};

#[test]
fn classifier_to_binding_named_definition_drift_refuses_without_a_scan_or_retry() {
    let data = Data::new();
    let definition = parse_query("SELECT Id FROM Items", ParseLimits::default()).unwrap();
    struct Drifting<'a> {
        data: &'a Data,
        definition: &'a Query,
        calls: Cell<usize>,
    }
    impl Source for Drifting<'_> {
        fn query(&self, _: &str) -> Result<Option<&Query>, QueryError> {
            let call = self.calls.get();
            self.calls.set(call + 1);
            Ok((call >= 2).then_some(self.definition))
        }
        fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
            self.data.columns(name)
        }
        fn scan(
            &self,
            _: &str,
            _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            panic!("metadata drift must fail before scanning");
        }
    }
    let source = Drifting {
        data: &data,
        definition: definition.query(),
        calls: Cell::new(0),
    };
    let query = parse_query("SELECT [Prompt] FROM Items", ParseLimits::default()).unwrap();
    assert_eq!(
        parameter_discovery_mode(
            query.query(),
            &source,
            Default::default(),
            &mut budget(),
            &|| false
        )
        .unwrap(),
        ParameterDiscoveryMode::Implicit
    );
    assert!(matches!(
        prepare_with_parameter_discovery(
            query.query(),
            &source,
            PreparationOptions::default(),
            &mut budget(),
            &|| false
        ),
        Err(QueryError::Unsupported)
    ));
    assert_eq!(
        source.calls.get(),
        3,
        "classifier, preparation precheck, then actual binding; no strict retry"
    );
    assert_eq!(data.scans.get(), 0);
}

#[test]
fn real_fields_and_later_eligible_aliases_win_over_repeated_implicit_names() {
    let data = Data::new();
    let query=parse_query("SELECT [Entry] AS InputValue,Id AS Entry,[entry] AS ResolvedAlias,Score AS Score FROM Items WHERE Id=1 ORDER BY Entry",ParseLimits::default()).unwrap();
    let prepared = prepare_with_parameter_discovery(
        query.query(),
        &data,
        PreparationOptions::default(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let prompts: Vec<_> = prepared.parameters().collect();
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "Entry");
    assert_eq!(prompts[0].origin, ParameterOrigin::Implicit);
    assert_eq!(data.scans.get(), 0);
    let result = prepared
        .execute_collections(
            &data,
            &[("Entry", Value::Text("input".into()))],
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![
            scalar(Value::Text("input".into())),
            scalar(Value::Int32(1)),
            scalar(Value::Int32(1)),
            scalar(Value::Int32(5))
        ]]
    );
}

#[test]
fn strict_named_domain_crosstab_and_authorized_external_queries_still_execute() {
    use crate::source::{ExternalSourceRequest, ResolvedExternalSource, SourceKey};
    let data = Data::new();
    let definition = parse_query("SELECT Id FROM Items", ParseLimits::default()).unwrap();
    struct Registry<'a> {
        data: &'a Data,
        definition: &'a Query,
        external_calls: Cell<usize>,
    }
    impl Source for Registry<'_> {
        fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
            Ok((name == "Saved").then_some(self.definition))
        }
        fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
            self.data.columns(name)
        }
        fn scan(
            &self,
            name: &str,
            visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            self.data.scan(name, visitor)
        }
        fn external_source(
            &self,
            request: ExternalSourceRequest<'_>,
        ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
            self.external_calls.set(self.external_calls.get() + 1);
            Ok((request.database() == "authorized")
                .then_some(ResolvedExternalSource::new(SourceKey::new(19), self.data)))
        }
    }
    let registry = Registry {
        data: &data,
        definition: definition.query(),
        external_calls: Cell::new(0),
    };
    for sql in [
        "SELECT Id FROM Saved ORDER BY Id",
        "SELECT Id FROM Items IN 'authorized' ORDER BY Id",
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let before = registry.external_calls.get();
        assert_eq!(
            parameter_discovery_mode(
                query.query(),
                &registry,
                Default::default(),
                &mut budget(),
                &|| false
            )
            .unwrap(),
            ParameterDiscoveryMode::Strict
        );
        assert_eq!(
            registry.external_calls.get(),
            before,
            "classification cannot activate an external binding"
        );
        let result = prepare(query.query(), &registry)
            .unwrap()
            .execute_collections(&registry, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![scalar(Value::Int32(1))], vec![scalar(Value::Int32(2))]]
        );
    }
    let query = parse_query(
        "SELECT DCount('*','Items') AS Total",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        parameter_discovery_mode(
            query.query(),
            &registry,
            Default::default(),
            &mut budget(),
            &|| false
        )
        .unwrap(),
        ParameterDiscoveryMode::Strict
    );
    let result = prepare(query.query(), &registry)
        .unwrap()
        .execute_collections(&registry, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(result.rows(), [vec![scalar(Value::Int64(2))]]);
    let query = parse_query(
        "TRANSFORM Count(*) SELECT Id FROM Items GROUP BY Id PIVOT Score IN (5,9)",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        parameter_discovery_mode(
            query.query(),
            &registry,
            Default::default(),
            &mut budget(),
            &|| false
        )
        .unwrap(),
        ParameterDiscoveryMode::Strict
    );
    let result = prepare(query.query(), &registry)
        .unwrap()
        .execute_collections(&registry, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(result.rows().len(), 2);
    assert_eq!(result.columns().len(), 3);
}

fn balanced_sum(level: usize, leaf: &str) -> String {
    if level == 0 {
        return leaf.into();
    }
    format!(
        "({}+{})",
        balanced_sum(level - 1, leaf),
        balanced_sum(level - 1, leaf)
    )
}

#[test]
fn paid_classifier_does_not_add_expression_depth_beyond_existing_preparation() {
    use crate::{BindingDepth, prepare_with_options};

    let data = Data::new();
    // Use the same caller-selected structural depth for the existing control,
    // classifier, discovery binder and execution; defaults remain unchanged.
    let preparation = PreparationOptions {
        binding_depth: BindingDepth::new(32).unwrap(),
        ..Default::default()
    };
    let execution = ExecutionOptions {
        binding_depth: preparation.binding_depth,
        ..Default::default()
    };
    let body = balanced_sum(7, "Score");
    let parsed = parse_query(
        &format!("SELECT {body} AS Total FROM Items WHERE Id=1"),
        ParseLimits::default(),
    )
    .unwrap();
    let existing = prepare_with_options(parsed.query(), &data, preparation)
        .unwrap()
        .execute_collections(&data, &[], execution)
        .unwrap();
    assert_eq!(existing.rows(), [vec![scalar(Value::Int32(640))]]);
    assert_eq!(
        parameter_discovery_mode(
            parsed.query(),
            &data,
            preparation.binding_depth,
            &mut budget(),
            &|| false
        )
        .unwrap(),
        ParameterDiscoveryMode::Implicit
    );
    let captured = prepare_with_parameter_discovery(
        parsed.query(),
        &data,
        preparation,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .execute_collections(&data, &[], execution)
    .unwrap();
    assert_eq!(captured.rows(), existing.rows());

    let body = balanced_sum(7, "[P]");
    let declared = parse_query(
        &format!("PARAMETERS P VALUE; SELECT {body} AS Total"),
        ParseLimits::default(),
    )
    .unwrap();
    let implicit = parse_query(&format!("SELECT {body} AS Total"), ParseLimits::default()).unwrap();
    let inputs = [("P", Value::Int32(5))];
    let existing = prepare_with_options(declared.query(), &data, preparation)
        .unwrap()
        .execute_collections(&data, &inputs, execution)
        .unwrap();
    assert_eq!(existing.rows(), [vec![scalar(Value::Int32(640))]]);
    let prepared = prepare_with_parameter_discovery(
        implicit.query(),
        &data,
        preparation,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(prepared.parameters().len(), 1);
    assert_eq!(
        prepared
            .execute_collections(&data, &inputs, execution)
            .unwrap()
            .rows(),
        existing.rows()
    );

    let body = balanced_sum(7, "Score");
    let parsed = parse_query(
        &format!("SELECT {body}+DCount('*','Items') AS Total FROM Items WHERE Id=1"),
        ParseLimits::default(),
    )
    .unwrap();
    let existing = prepare_with_options(parsed.query(), &data, preparation)
        .unwrap()
        .execute_collections(&data, &[], execution)
        .unwrap();
    assert_eq!(existing.rows(), [vec![scalar(Value::Int64(642))]]);
    assert_eq!(
        parameter_discovery_mode(
            parsed.query(),
            &data,
            preparation.binding_depth,
            &mut budget(),
            &|| false
        )
        .unwrap(),
        ParameterDiscoveryMode::Strict
    );
}
