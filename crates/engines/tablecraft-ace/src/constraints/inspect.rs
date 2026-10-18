use super::{ConstraintReport, RelationshipCheck, fields, relationships, rules};
use crate::{
    InspectError, PropertyGroupKind, header::check, inspect_relationships,
    inspect_table_properties, read_table,
};
use std::io::{Read, Seek};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

/// Explicit bounded record audit of Required/AllowZeroLength and qualified
/// one-to-ten-column Int32 foreign keys touching one table. Only bounded pure field/table
/// validation expressions run; unqualified rules are reported unsupported. Structural
/// index checks remain the separate check_table API. No writes/cascades execute.
pub fn check_table_constraints<R: Read + Seek>(
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
        let status = relationships::check_relationship(
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
            status,
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
