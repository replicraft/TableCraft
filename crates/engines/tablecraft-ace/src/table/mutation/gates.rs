#[cfg(feature = "ace16-write-preview")]
mod defaults;
#[cfg(feature = "ace16-write-preview")]
mod display;
#[cfg(feature = "ace16-write-preview")]
mod historical;

use super::super::indexes::FOREIGN_INDEX;
#[cfg(feature = "ace16-write-preview")]
use super::guards::fields::physical::SourceWitness;
use super::{EditError, WriteBlocker};
#[cfg(feature = "ace16-write-preview")]
use crate::format::{
    data_type::INT32,
    layout::{
        AUTO_NUMBER_FLAG, AUTO_NUMBER_GUID_FLAG, CALCULATED_EXT_MASK, FIXED_FLAG, UPDATABLE_FLAG,
    },
};
use crate::{
    ConstraintReport, FileFormat, IndexCheckStatus, NativeImage, PayloadPresence,
    PropertyGroupKind, RelationshipCheckStatus, RuleCheckStatus, TableCheck, check_table,
    check_table_constraints, format::page::PAGE_NUMBER_MASK, header::check,
    inspect_foreign_index_links, inspect_table_properties,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

// PropertyMapImpl.DDL_FLAG in the pinned Jackcess reference; all other stored
// property flag bits remain unqualified for native mutation.
const SUPPORTED_PROPERTY_FLAG_MASK: u8 = 0x01;

#[cfg(feature = "ace16-write-preview")]
enum PayloadAdmission<'a> {
    Inert,
    BeforeChange(&'a SourceWitness),
}

pub(super) fn before(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    before_with(
        image,
        table,
        Admission::Update,
        #[cfg(feature = "ace16-write-preview")]
        PayloadAdmission::Inert,
        budget,
        cancelled,
    )
}

#[cfg(feature = "ace16-write-preview")]
pub(super) fn before_delete(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    before_with(
        image,
        table,
        Admission::Delete,
        #[cfg(feature = "ace16-write-preview")]
        PayloadAdmission::Inert,
        budget,
        cancelled,
    )
}

#[cfg(feature = "ace16-write-preview")]
pub(super) fn before_slot(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    before_with(
        image,
        table,
        Admission::SlotUpdate,
        #[cfg(feature = "ace16-write-preview")]
        PayloadAdmission::Inert,
        budget,
        cancelled,
    )
}

#[cfg(feature = "ace16-write-preview")]
pub(super) fn before_fixed_update(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    before_with(
        image,
        table,
        Admission::FixedUpdate,
        #[cfg(feature = "ace16-write-preview")]
        PayloadAdmission::Inert,
        budget,
        cancelled,
    )
}

#[cfg(feature = "ace16-write-preview")]
pub(super) fn before_insert(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    before_with(
        image,
        table,
        Admission::Insert,
        #[cfg(feature = "ace16-write-preview")]
        PayloadAdmission::Inert,
        budget,
        cancelled,
    )
}

#[cfg(feature = "ace16-write-preview")]
pub(super) fn before_incrementing_insert(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    before_with(
        image,
        table,
        Admission::IncrementingInsert,
        PayloadAdmission::Inert,
        budget,
        cancelled,
    )
}

#[cfg(feature = "ace16-write-preview")]
pub(super) fn before_prepared_fields(
    image: &NativeImage,
    table: &str,
    witness: &SourceWitness,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    let checked = before_with(
        image,
        table,
        Admission::FixedUpdate,
        PayloadAdmission::BeforeChange(witness),
        budget,
        cancelled,
    )?;
    super::fixed::extended::protect_source(
        image,
        &checked,
        checked
            .schema()
            .columns()
            .iter()
            .map(|column| column.name()),
        budget,
        cancelled,
    )?;
    Ok(checked)
}

enum Admission {
    Update,
    #[cfg(feature = "ace16-write-preview")]
    FixedUpdate,
    #[cfg(feature = "ace16-write-preview")]
    Delete,
    #[cfg(feature = "ace16-write-preview")]
    SlotUpdate,
    #[cfg(feature = "ace16-write-preview")]
    Insert,
    #[cfg(feature = "ace16-write-preview")]
    IncrementingInsert,
}

fn before_with(
    image: &NativeImage,
    table: &str,
    admission: Admission,
    #[cfg(feature = "ace16-write-preview")] payload: PayloadAdmission<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    let scope = image.catalog().catalog().scope();
    let structural = check_table(&mut image.reader(), scope, table, budget, cancelled)?;
    let schema = structural.schema();
    // French physical read qualification does not authorize mutation profiles.
    if structural.indexes().iter().any(|index| {
        index.index().columns().iter().any(|key| {
            schema.columns().iter().any(|column| {
                column.number() == key.number()
                    && column
                        .text_sort_order()
                        .is_some_and(|order| order.is_french_legacy())
            })
        })
    }) {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }

    if blocks_binary_update(&admission)
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
    let historical_columns = schema.columns().len() != schema.layout.max_columns
        || schema
            .columns()
            .iter()
            .enumerate()
            .any(|(number, column)| column.number() != number);
    if historical_columns {
        #[cfg(feature = "ace16-write-preview")]
        if matches!(admission, Admission::FixedUpdate) {
            historical::qualify(image, schema, budget, cancelled)?;
        } else {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        #[cfg(not(feature = "ace16-write-preview"))]
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    #[cfg(feature = "ace16-write-preview")]
    if matches!(admission, Admission::Insert | Admission::IncrementingInsert) {
        let generated = if matches!(admission, Admission::IncrementingInsert) {
            Some(super::insert::primary_column::resolve(
                &structural,
                budget,
                cancelled,
            )?)
        } else {
            None
        };
        for (position, column) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            if generated == Some(position) {
                if column.native_type() != INT32
                    || !column.fixed()
                    || column.storage_length() != size_of::<i32>()
                    || column.flags() != (FIXED_FLAG | UPDATABLE_FLAG | AUTO_NUMBER_FLAG)
                    || column.extended_flags() != 0
                {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
            } else if column.flags() & (AUTO_NUMBER_FLAG | AUTO_NUMBER_GUID_FLAG) != 0 {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            if column.extended_flags() & CALCULATED_EXT_MASK != 0
                || column.complex_type_id().is_some()
            {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
    }
    source_origin(image, schema, 0, budget, cancelled)?;
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
            // Complete physical membership permits the separate DELETE/INSERT
            // consumers, whose planners still qualify their own key profiles.
            // Ordinary/slot/cascade UPDATE retains its blocker. The separately
            // qualified fixed UPDATE consumer binds the complete omitted set.
            || (!has_ignored_index_consumer(&admission)
                && index.index().constraints().is_some_and(|constraints| constraints.ignore_nulls()))
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
    #[cfg(feature = "ace16-write-preview")]
    match payload {
        PayloadAdmission::Inert => inert_payloads(image, schema, budget, cancelled)?,
        PayloadAdmission::BeforeChange(witness) => {
            witness.require(image, schema, budget, cancelled)?
        }
    }
    #[cfg(not(feature = "ace16-write-preview"))]
    inert_payloads(image, schema, budget, cancelled)?;
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
            // These exact ACE display declarations leave stored values and
            // physical scale unchanged; all other property gates still apply.
            #[cfg(feature = "ace16-write-preview")]
            if matches!(
                image.catalog().header().format(),
                FileFormat::Ace2016 | FileFormat::Ace2019
            ) && display::requested(property.name())
            {
                display::qualify(structural.schema(), group, property, budget, cancelled)?;
                continue;
            }

            // Complete supplied values do not invoke field defaults. Qualify
            // their inert declaration without adding omission/evaluation semantics.
            #[cfg(feature = "ace16-write-preview")]
            if matches!(admission, Admission::Insert | Admission::IncrementingInsert)
                && property.name().eq_ignore_ascii_case("DefaultValue")
            {
                defaults::qualify(structural.schema(), group, property, budget, cancelled)?;
            }
            // Captions/descriptions and rejection text are passive.
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

fn has_ignored_index_consumer(admission: &Admission) -> bool {
    match admission {
        #[cfg(feature = "ace16-write-preview")]
        Admission::Delete
        | Admission::Insert
        | Admission::IncrementingInsert
        | Admission::FixedUpdate => true,
        _ => false,
    }
}

fn blocks_binary_update(admission: &Admission) -> bool {
    match admission {
        Admission::Update => true,
        #[cfg(feature = "ace16-write-preview")]
        Admission::FixedUpdate => true,
        #[cfg(feature = "ace16-write-preview")]
        _ => false,
    }
}

// This is source identity/payload admission, not a macro-absence certificate.
pub(super) fn source_origin(
    image: &NativeImage,
    schema: &crate::TableSchema,
    allowed_flags: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let identity_work = image
        .catalog()
        .origins()
        .len()
        .checked_mul(2)
        .ok_or(crate::InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, identity_work as u64)
        .map_err(crate::InspectError::from)?;
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == schema.object_id())
        .ok_or(crate::InspectError::Malformed)?;
    if origin.flags() & !allowed_flags != 0
        || image.catalog().origins().iter().any(|other| {
            (other.flags() & !allowed_flags != 0
                || (other.object_id() != origin.object_id()
                    && (allowed_flags != 0 || other.type_code() == origin.type_code())))
                && other.native_id() as u32 & PAGE_NUMBER_MASK == schema.definition_page()
        })
    {
        return Err(EditError::Unsupported(WriteBlocker::Table));
    }
    Ok(())
}

pub(super) fn inert_payloads(
    image: &NativeImage,
    schema: &crate::TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    // A NULL local Lv/LvModule/LvExtra does not qualify separately owned catalog
    // objects. Keep their potentially active/unknown behavior outside this writer.
    if !image
        .inspect_object_children(schema.object_id(), budget, cancelled)?
        .is_empty()
    {
        return Err(EditError::Unsupported(WriteBlocker::ActiveContent));
    }
    let payloads = image.inspect_object_payloads(schema.object_id(), budget, cancelled)?;
    if [payloads.body(), payloads.module(), payloads.extra()]
        .iter()
        .any(|presence| *presence != PayloadPresence::Absent)
    {
        return Err(EditError::Unsupported(WriteBlocker::ActiveContent));
    }
    Ok(())
}
