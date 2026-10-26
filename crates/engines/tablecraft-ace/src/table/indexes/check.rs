//! Explicit structural/row and qualified physical index-constraint coverage.
use super::super::TableSnapshot;
use super::super::{TableSchema, read_table};
use super::key::KeyWork;
use super::{
    IndexConstraints, IndexProjection, IndexProjectionSelection, NativeIndex,
    inspect_table_indexes, key::KeyProfile, profile::IndexProfile, read::IgnoredMembershipCheck,
    read_table_index_projection,
};
use crate::InspectError;
use projection::CheckProjection;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

mod ignored_nulls;
mod projection;

/// Coverage of one native index in a completed table check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexCheckStatus {
    /// Complete qualified traversal, root/key validation and index checks.
    Checked,
    /// Index pages/rows were not checked because the profile is unqualified.
    Unsupported,
}
/// Native definition and explicit validation coverage; no payload Debug/Serde.
pub struct IndexCheck {
    index: NativeIndex,
    status: IndexCheckStatus,
}
impl IndexCheck {
    /// Native index metadata, not a mutation handle.
    pub fn index(&self) -> &NativeIndex {
        &self.index
    }
    /// Whether the qualified complete physical-index reader checked this index.
    pub const fn status(&self) -> IndexCheckStatus {
        self.status
    }
}
/// Completed sequential scalar read and coverage for each inventoried index.
/// Does not qualify field properties, foreign keys, skipped profiles, transactions,
/// active content, shared-file isolation or the database as a whole.
pub struct TableCheck {
    schema: TableSchema,
    rows: usize,
    indexes: Vec<IndexCheck>,
}
impl TableCheck {
    /// Native scalar table metadata.
    pub fn schema(&self) -> &TableSchema {
        &self.schema
    }
    /// Live roots decoded sequentially and matched to the declared count.
    pub const fn rows(&self) -> usize {
        self.rows
    }
    /// Native definition order, including explicitly unverified profiles.
    pub fn indexes(&self) -> &[IndexCheck] {
        &self.indexes
    }
}
/// Check one explicit stable/offline scalar table sequentially and through every
/// qualified Boolean/numeric/legacy DateTime/GUID index. Unsupported profiles are
/// reported, not marked checked. Qualified ignore-NULL indexes compare complete
/// physical membership against independently counted sequential roots whose key
/// components are not all NULL. All roots still participate in required-key checks.
/// Failures in metadata/selected traversal return no partial report.
/// Caller budgets accumulate across every read, without refunds or retries.
pub fn check_table<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, InspectError> {
    let _span = tracing::debug_span!("native.table.check").entered();
    let mut inspection = inspect_table_indexes(source, scope, table, budget, cancelled)?;
    match super::links::qualify(source, scope, &mut inspection, budget, cancelled) {
        Ok(()) | Err(InspectError::Unsupported) => {}
        Err(error) => return Err(error),
    }
    let sequential = read_table(source, scope, table, budget, cancelled)?;
    let columns = inspection.schema.scalar_columns_with_budget(budget)?;
    let rows = sequential.rows().len();
    if rows != inspection.schema.declared_rows() as usize
        || columns != sequential.columns()
        || inspection.schema.object_id() != sequential.schema().object_id()
        || inspection.schema.definition_page() != sequential.schema().definition_page()
    {
        return Err(InspectError::Malformed);
    }
    let ignored_populations =
        ignored_nulls::populations(&inspection, &sequential, budget, cancelled)?;
    drop(sequential);
    budget.charge_allocation(
        inspection.indexes.len() as u64,
        size_of::<IndexCheck>() as u64,
    )?;
    let mut indexes = Vec::new();
    indexes
        .try_reserve_exact(inspection.indexes.len())
        .map_err(|_| InspectError::Allocation)?;
    for (position, index) in inspection.indexes.into_iter().enumerate() {
        tick(budget, cancelled)?;
        let eligible_rows = ignored_populations
            .as_ref()
            .and_then(|populations| populations[position]);
        let profile = if eligible_rows.is_some() {
            IndexProfile::for_check(&index, &inspection.schema)
        } else {
            IndexProfile::new(&index, &inspection.schema)
        };
        let status = match profile {
            Ok(profile) => {
                let projection =
                    CheckProjection::new(&inspection.schema, &index, budget, cancelled)?;
                let indexed = if let Some(eligible_rows) = eligible_rows {
                    super::read::read_ignored_membership_check(
                        source,
                        scope,
                        table,
                        index.name(),
                        IgnoredMembershipCheck {
                            columns: projection.names(),
                            eligible_rows,
                        },
                        budget,
                        cancelled,
                    )?
                } else {
                    read_table_index_projection(
                        source,
                        scope,
                        table,
                        index.name(),
                        IndexProjection {
                            columns: projection.names(),
                            selection: IndexProjectionSelection::All,
                        },
                        budget,
                        cancelled,
                    )?
                };
                if indexed.rows().len() != eligible_rows.unwrap_or(rows)
                    || indexed.schema().scalar_columns_with_budget(budget)? != columns
                    || indexed.schema().object_id() != inspection.schema.object_id()
                    || indexed.schema().definition_page() != inspection.schema.definition_page()
                {
                    return Err(InspectError::Malformed);
                }
                let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
                let checked = match profile {
                    IndexProfile::EncodedTuple(profile) => check_keys(
                        &indexed,
                        &projection,
                        constraints,
                        profile,
                        budget,
                        cancelled,
                    ),
                    IndexProfile::Text(profile) => check_keys(
                        &indexed,
                        &projection,
                        constraints,
                        profile,
                        budget,
                        cancelled,
                    ),
                    IndexProfile::Binary(profile) => check_keys(
                        &indexed,
                        &projection,
                        constraints,
                        profile,
                        budget,
                        cancelled,
                    ),
                    IndexProfile::Single(profile) => check_keys(
                        &indexed,
                        &projection,
                        constraints,
                        profile,
                        budget,
                        cancelled,
                    ),
                    IndexProfile::Composite(profile) => check_keys(
                        &indexed,
                        &projection,
                        constraints,
                        profile,
                        budget,
                        cancelled,
                    ),
                    IndexProfile::Guid(profile) => check_keys(
                        &indexed,
                        &projection,
                        constraints,
                        profile,
                        budget,
                        cancelled,
                    ),
                    IndexProfile::GuidComposite(profile) => check_keys(
                        &indexed,
                        &projection,
                        constraints,
                        profile,
                        budget,
                        cancelled,
                    ),
                    IndexProfile::Decimal(profile) => check_keys(
                        &indexed,
                        &projection,
                        constraints,
                        profile,
                        budget,
                        cancelled,
                    ),
                    IndexProfile::DecimalComposite(profile) => check_keys(
                        &indexed,
                        &projection,
                        constraints,
                        profile,
                        budget,
                        cancelled,
                    ),
                };
                // Full indexed reads prove distinct live roots by key/row order
                // and decoded key agreement. The complete sequential count then
                // establishes root coverage on the caller's stable source. An
                // ignored-NULL index instead proves the independently counted
                // population whose key components are not all NULL. This does
                // not make its returned records a complete table.
                match checked {
                    Ok(()) => IndexCheckStatus::Checked,
                    Err(InspectError::Unsupported) => IndexCheckStatus::Unsupported,
                    Err(error) => return Err(error),
                }
            }
            Err(InspectError::Unsupported) => IndexCheckStatus::Unsupported,
            Err(error) => return Err(error),
        };
        indexes.push(IndexCheck { index, status });
    }
    tick(budget, cancelled)?;
    tracing::debug!(
        rows,
        indexes = indexes.len(),
        checked = indexes
            .iter()
            .filter(|i| i.status == IndexCheckStatus::Checked)
            .count(),
        "native table check completed"
    );
    Ok(TableCheck {
        schema: inspection.schema,
        rows,
        indexes,
    })
}
fn check_keys<P: KeyProfile>(
    table: &TableSnapshot,
    projection: &CheckProjection<'_>,
    constraints: IndexConstraints,
    profile: P,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let mut previous = None;
    for row in table.rows() {
        tick(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            profile.components().saturating_sub(1) as u64,
        )?;
        let key = profile.row_key(&projection.row(row), &mut KeyWork { budget, cancelled })?;
        if constraints.unique() && !profile.unique_key_is_exact(&key) {
            return Err(InspectError::Unsupported);
        }
        let (all_null, any_null) = profile.constraint_nulls(&key)?;
        constraints.check_tuple(&mut previous, (!all_null).then_some(key), any_null)?;
    }
    Ok(())
}
fn tick(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), InspectError> {
    if cancelled() {
        return Err(InspectError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}
