use super::{ConstraintReport, RelationshipCheck, fields, relationships, rules};
use crate::{
    InspectError, PropertyGroupKind, header::check, inspect_relationships,
    inspect_table_properties, read_table,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

/// Explicit bounded record audit of Required/AllowZeroLength and qualified
/// one-to-ten-column scalar foreign keys touching one table. Paired storage
/// types must match; Boolean coverage remains read-only. Only bounded pure field/table
/// validation expressions run; unqualified rules are reported unsupported. Structural
/// index checks remain the separate check_table API. No writes/cascades execute.
pub fn check_table_constraints<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ConstraintReport, InspectError> {
    let _span = tracing::debug_span!("native.constraints.check").entered();
    let properties = inspect_table_properties(source, scope, name, budget, cancelled)?;
    let target = read_table(source, scope, name, budget, cancelled)?;
    let fields = fields::check_fields(&target, &properties, budget, cancelled)?;
    let table_validation = rules::audit(
        &target,
        properties.group(PropertyGroupKind::Table, ""),
        None,
        budget,
        cancelled,
    )?;
    let declared = inspect_relationships(source, scope, budget, cancelled)?;
    let mut checks = Vec::new();
    for relationship in declared {
        check(budget, cancelled)?;
        let declaration = relationship.declaration();
        if !declaration
            .parent_table()
            .as_str()
            .eq_ignore_ascii_case(target.schema().name())
            && !declaration
                .child_table()
                .as_str()
                .eq_ignore_ascii_case(target.schema().name())
        {
            continue;
        }
        let coverage = relationships::check_relationship(
            source,
            scope,
            &relationship,
            &target,
            budget,
            cancelled,
        )?;
        budget.charge_allocation(1, size_of::<RelationshipCheck>() as u64)?;
        checks
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        checks.push(RelationshipCheck {
            relationship,
            status: coverage.status,
            existing_mutation_key_profile: coverage.existing_mutation_key_profile,
        });
    }
    check(budget, cancelled)?;
    Ok(ConstraintReport {
        name: fields::copy_name(target.schema().name(), budget)?,
        rows: target.rows().len(),
        fields,
        table_validation,
        relationships: checks,
    })
}

/// Internal ordinary-projection constraint audit for schema-stable preservation.
/// The caller separately certifies every excluded field and its declarations.
#[cfg(feature = "ace16-write-preview")]
pub(crate) fn check_scalar_projection(
    table: &crate::TableSnapshot,
    properties: &crate::TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let fields = fields::check_fields(table, properties, budget, cancelled)?;
    if fields.iter().any(|field| {
        [field.required(), field.empty_text(), field.validation()]
            .contains(&super::RuleCheckStatus::Unsupported)
    }) || rules::audit(
        table,
        properties.group(PropertyGroupKind::Table, ""),
        None,
        budget,
        cancelled,
    )? == super::RuleCheckStatus::Unsupported
    {
        return Err(InspectError::Unsupported);
    }
    check(budget, cancelled)?;
    Ok(())
}
