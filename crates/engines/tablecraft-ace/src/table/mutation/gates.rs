use super::{EditError, WriteBlocker};
use crate::{
    ConstraintReport, IndexCheckStatus, NativeImage, PropertyGroupKind, RelationshipCheckStatus,
    RuleCheckStatus, TableCheck, check_table, check_table_constraints,
    format::page::PAGE_NUMBER_MASK, header::check, inspect_table_properties,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn before(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    let scope = image.catalog().catalog().scope();
    let structural = check_table(&mut image.reader(), scope, table, budget, cancelled)?;
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == structural.schema().object_id())
        .ok_or(crate::InspectError::Malformed)?;
    if origin.flags() != 0
        || image.catalog().origins().iter().any(|other| {
            other.flags() != 0
                && other.native_id() as u32 & PAGE_NUMBER_MASK
                    == structural.schema().definition_page()
        })
    {
        return Err(EditError::Unsupported(WriteBlocker::Table));
    }
    if structural
        .indexes()
        .iter()
        .any(|index| index.status() != IndexCheckStatus::Checked)
    {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let properties =
        inspect_table_properties(&mut image.reader(), scope, table, budget, cancelled)?;
    for group in properties.groups() {
        check(budget, cancelled)?;
        if !matches!(
            group.kind(),
            PropertyGroupKind::Table | PropertyGroupKind::Column
        ) || (group.kind() == PropertyGroupKind::Table && !group.name().is_empty())
            || (group.kind() == PropertyGroupKind::Column
                && !structural
                    .schema()
                    .columns()
                    .iter()
                    .any(|column| column.name().eq_ignore_ascii_case(group.name())))
        {
            return Err(EditError::Unsupported(WriteBlocker::Properties));
        }
        for property in group.properties() {
            check(budget, cancelled)?;
            // Existing records are updated, so DefaultValue is retained but not
            // evaluated. Captions/descriptions and rejection text are passive.
            if property.flags() > 1
                || ![
                    "Required",
                    "AllowZeroLength",
                    "ValidationRule",
                    "ValidationText",
                    "DefaultValue",
                    "Caption",
                    "Description",
                ]
                .iter()
                .any(|name| name.eq_ignore_ascii_case(property.name()))
            {
                return Err(EditError::Unsupported(WriteBlocker::Properties));
            }
        }
    }
    qualified(&check_table_constraints(
        &mut image.reader(),
        scope,
        table,
        budget,
        cancelled,
    )?)?;
    Ok(structural)
}

pub(super) fn qualified(report: &ConstraintReport) -> Result<(), EditError> {
    if report.table_validation() == RuleCheckStatus::Unsupported
        || report.fields().iter().any(|field| {
            [field.required(), field.empty_text(), field.validation()]
                .contains(&RuleCheckStatus::Unsupported)
        })
        || report
            .relationships()
            .iter()
            .any(|relationship| relationship.status() == RelationshipCheckStatus::Unsupported)
    {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    Ok(())
}
