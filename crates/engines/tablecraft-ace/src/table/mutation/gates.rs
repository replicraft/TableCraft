use super::super::indexes::FOREIGN_INDEX;
use super::{EditError, WriteBlocker};
use crate::{
    ConstraintReport, FileFormat, IndexCheckStatus, NativeImage, PayloadPresence,
    PropertyGroupKind, RelationshipCheckStatus, RuleCheckStatus, TableCheck, check_table,
    check_table_constraints, format::page::PAGE_NUMBER_MASK, header::check,
    inspect_foreign_index_links, inspect_table_properties,
};
use tablecraft_core::limits::ResourceBudget;

// PropertyMapImpl.DDL_FLAG in the pinned Jackcess reference; all other stored
// property flag bits remain unqualified for native mutation.
const SUPPORTED_PROPERTY_FLAG_MASK: u8 = 0x01;

pub(super) fn before(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    before_with(image, table, Admission::Update, budget, cancelled)
}

#[cfg(feature = "ace16-write-preview")]
pub(super) fn before_delete(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    before_with(image, table, Admission::Delete, budget, cancelled)
}

enum Admission {
    Update,
    #[cfg(feature = "ace16-write-preview")]
    Delete,
}

fn before_with(
    image: &NativeImage,
    table: &str,
    admission: Admission,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    let scope = image.catalog().catalog().scope();
    let structural = check_table(&mut image.reader(), scope, table, budget, cancelled)?;
    let schema = structural.schema();
    if matches!(admission, Admission::Update)
        && structural.indexes().iter().any(|index| {
            index.index().columns().iter().any(|key| {
                schema.columns().iter().any(|column| {
                    column.number() == key.number()
                        && column.native_type() == crate::format::data_type::SHORT_BINARY
                })
            })
        })
    {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    // Deleted-column read qualification does not qualify edits of historical storage.
    if schema.columns().len() != schema.layout.max_columns
        || schema
            .columns()
            .iter()
            .enumerate()
            .any(|(number, column)| column.number() != number)
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == structural.schema().object_id())
        .ok_or(crate::InspectError::Malformed)?;
    if origin.flags() != 0
        || image.catalog().origins().iter().any(|other| {
            (other.flags() != 0
                || (other.object_id() != origin.object_id()
                    && other.type_code() == origin.type_code()))
                && other.native_id() as u32 & PAGE_NUMBER_MASK
                    == structural.schema().definition_page()
        })
    {
        return Err(EditError::Unsupported(WriteBlocker::Table));
    }
    // ACE families reach this helper only through engineering-preview entry
    // points. A completed foreign check includes reciprocal/catalog agreement;
    // final relationship membership must still pass after the complete edit.
    // Keep the ordinary Jet 4 writer's foreign-index blocker unchanged.
    let foreign_preview = matches!(
        image.catalog().header().format(),
        FileFormat::Ace2016 | FileFormat::Ace2019
    );
    if structural.indexes().iter().any(|index| {
        index.status() != IndexCheckStatus::Checked
            // Read checks can now qualify the non-NULL physical membership of
            // an ignored-NULL index. Mutation eligibility is unchanged.
            || index.index().constraints().is_some_and(|constraints| constraints.ignore_nulls())
            || (!foreign_preview && index.index().native_type() == FOREIGN_INDEX)
    }) {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    if foreign_preview
        && !structural
            .indexes()
            .iter()
            .any(|index| index.index().native_type() == FOREIGN_INDEX)
    {
        // Ordinary reads need not resolve a relationship graph when no foreign
        // indexes are selected. A preview must also reject an enforced catalog
        // edge whose native foreign declarations were removed or disguised.
        inspect_foreign_index_links(&mut image.reader(), scope, table, budget, cancelled)?;
    }
    // A NULL local Lv/LvModule/LvExtra does not qualify separately owned catalog
    // objects. Keep their potentially active/unknown behavior outside this writer.
    if !image
        .inspect_object_children(structural.schema().object_id(), budget, cancelled)?
        .is_empty()
    {
        return Err(EditError::Unsupported(WriteBlocker::ActiveContent));
    }
    let payloads =
        image.inspect_object_payloads(structural.schema().object_id(), budget, cancelled)?;
    if [payloads.body(), payloads.module(), payloads.extra()]
        .iter()
        .any(|presence| *presence != PayloadPresence::Absent)
    {
        return Err(EditError::Unsupported(WriteBlocker::ActiveContent));
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
            if property.flags() & !SUPPORTED_PROPERTY_FLAG_MASK != 0
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
        // Record membership can be qualified independently of mutation. Keep
        // newly readable key families outside both before/after write gates.
        || report.relationships().iter().any(|relationship| {
            relationship.status() == RelationshipCheckStatus::Unsupported
                || !relationship.existing_mutation_key_profile()
        })
    {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    Ok(())
}
