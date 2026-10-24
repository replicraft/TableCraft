//! Qualified calculated-field declarations without stored record access.
use super::read::qualified_kind;
use crate::{
    InspectError, NativeColumn, NativeReadSource, TableFeatures, header::check,
    inspect_table_features,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

/// Inert qualified declaration, independent of cached values and their freshness.
pub struct CalculationDeclaration {
    features: TableFeatures,
    position: usize,
    kind: ScalarType,
}

impl CalculationDeclaration {
    /// Original field and property metadata. No formula has executed.
    pub fn features(&self) -> &TableFeatures {
        &self.features
    }

    /// Qualified native calculated result declaration, not an ordinary field codec.
    pub fn column(&self) -> &NativeColumn {
        &self.features.schema().columns()[self.position]
    }

    /// Native declared stored result carrier. Expression promotion remains separate.
    pub const fn data_type(&self) -> ScalarType {
        self.kind
    }

    /// Exact original expression, retained without normalization or evaluation.
    pub fn expression(&self) -> Result<&str, InspectError> {
        self.features
            .columns()
            .nth(self.position)
            .ok_or(InspectError::Malformed)?
            .expression()?
            .ok_or(InspectError::Malformed)
    }
}

/// Inspect one qualified ACE 16-family calculated declaration without reading rows.
/// Uses the same profile, wrapper geometry and typed property gates as explicit
/// cached reads, but neither inspects cache payloads nor proves their freshness.
pub fn inspect_calculation_declaration<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CalculationDeclaration, InspectError> {
    let _span = tracing::debug_span!("native.calculation.declaration").entered();
    check(budget, cancelled)?;
    let features = inspect_table_features(source, scope, table, budget, cancelled)?;
    let position = features
        .schema()
        .columns()
        .iter()
        .position(|field| field.name().eq_ignore_ascii_case(column))
        .ok_or(InspectError::UnknownColumn)?;
    let field = features
        .columns()
        .nth(position)
        .ok_or(InspectError::Malformed)?;
    let kind = qualified_kind(&field)?;
    check(budget, cancelled)?;
    Ok(CalculationDeclaration {
        features,
        position,
        kind,
    })
}
