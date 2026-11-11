//! One validated saved-field declaration lowers without a synthetic table snapshot.
use crate::{
    Error,
    editing::{EditError, FieldAdditionOrigin},
};
use std::num::NonZeroU8;
use tablecraft_ace::{CreateColumn, RetainedFieldAdditionDefinition, RetainedFieldAdditionType};
use tablecraft_builders::table::TypedFieldAdditionDeclaration;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

fn column<'a>(
    name: &'a str,
    declaration: TypedFieldAdditionDeclaration,
    default: Option<&'a Value>,
) -> Result<CreateColumn<'a>, Error> {
    let descriptor = declaration.descriptor();
    let mut column = match descriptor.scalar_type() {
        ScalarType::Text => CreateColumn::short_text(
            name,
            usize::from(
                declaration
                    .short_text_units()
                    .ok_or(EditError::InvalidInput)?
                    .get(),
            ),
        )?
        .with_allow_zero_length(true)?
        .with_unicode_compression(declaration.unicode_compression())?,
        ScalarType::Decimal => CreateColumn::decimal(
            name,
            descriptor
                .decimal_constraints()
                .ok_or(EditError::InvalidInput)?,
        ),
        kind => CreateColumn::new(name, kind),
    }
    .with_required(false);
    if let Some(default) = default {
        column = column.with_default_literal(default)?;
    }
    Ok(column)
}

pub(super) fn capture(
    name: &str,
    declaration: TypedFieldAdditionDeclaration,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RetainedFieldAdditionDefinition, Error> {
    let default = super::super::declaration_defaults::for_type(declaration.descriptor())?;
    Ok(RetainedFieldAdditionDefinition::capture(
        column(name, declaration, default.as_ref())?,
        budget,
        cancelled,
    )?)
}

pub(super) fn target(
    declaration: TypedFieldAdditionDeclaration,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RetainedFieldAdditionType, Error> {
    let default = super::super::declaration_defaults::for_type(declaration.descriptor())?;
    // Native type capture deliberately ignores the name; advice has no synthetic
    // identifier whose collision could disable a valid type before user input.
    Ok(RetainedFieldAdditionType::capture(
        column("", declaration, default.as_ref())?,
        budget,
        cancelled,
    )?)
}

pub(super) fn captured(
    definition: &RetainedFieldAdditionDefinition,
) -> Result<TypedFieldAdditionDeclaration, Error> {
    let column = definition.column();
    let declaration = match column.scalar_type() {
        ScalarType::Text => TypedFieldAdditionDeclaration::short_text(
            u8::try_from(
                column
                    .max_text_utf16_units()
                    .ok_or(EditError::InvalidInput)?,
            )
            .ok()
            .and_then(NonZeroU8::new)
            .ok_or(EditError::InvalidInput)?,
            column.unicode_compression(),
        ),
        ScalarType::Decimal => {
            let constraints = column
                .decimal_constraints()
                .ok_or(EditError::InvalidInput)?;
            TypedFieldAdditionDeclaration::decimal(constraints.precision(), constraints.scale())
                .map_err(|_| EditError::InvalidInput)?
        }
        kind => TypedFieldAdditionDeclaration::fixed(kind).map_err(|_| EditError::InvalidInput)?,
    };
    Ok(declaration)
}

pub(super) fn captured_declared(column: CreateColumn<'_>) -> Result<FieldAdditionOrigin, Error> {
    if column.scalar_type() != ScalarType::Currency
        || column.required().is_some()
        || column.allow_zero_length().is_some()
        || column.default_literal().is_some()
        || column.description().is_some()
        || column.primary_key_name().is_some()
        || column.incrementing_autonumber()
        || column.unicode_compression()
    {
        return Err(Error::SchemaSqlUnsupported);
    }
    Ok(FieldAdditionOrigin::SqlDeclared {
        scalar_type: column.scalar_type(),
    })
}

#[cfg(test)]
mod declared_tests;
