//! Strict original LVAL declaration coverage for target-local admission only.
use super::refused;
use crate::{
    EditError, NativeImage, TableSchema,
    format::data_type::{LONG_TEXT, OLE},
    header::check,
    table::{long_value::maps::Declarations, mutation::map_authority},
};
use tablecraft_core::limits::ResourceBudget;

/// Every local table contributes its actual declaration metadata to the same
/// native census used by physical record protection. This is not a replacement
/// for Source protection, live-LVAL tracing, or map-storage exclusivity.
pub(super) fn capture(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.table.no_required_programs.declarations").entered();
    check(budget, cancelled)?;
    let census = map_authority::capture_catalog(
        image,
        budget,
        cancelled,
        |schema, _indexes, declarations, reader| {
            validate(schema, declarations, reader.budget, reader.cancelled)
        },
    )?;
    drop(census);
    check(budget, cancelled)?;
    Ok(())
}

/// `complete` describes framing in the shared reader. This caller additionally
/// requires a bijection with every actual native Memo/OLE field declaration.
/// Initialization's absent/zero-pair policies are deliberately not used here.
pub(super) fn validate(
    schema: &TableSchema,
    declarations: &Declarations,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if !declarations.complete {
        return Err(refused());
    }

    let mut required = 0_usize;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        for prior in &schema.columns()[..position] {
            check(budget, cancelled)?;
            if prior.number() == field.number() {
                return Err(refused());
            }
        }
        if matches!(field.native_type(), LONG_TEXT | OLE) {
            required += 1;
        }
    }
    if declarations.entries.len() != required {
        return Err(refused());
    }

    for (position, declaration) in declarations.entries.iter().enumerate() {
        check(budget, cancelled)?;
        if declaration.references.owned() == 0 || declaration.references.candidate() == 0 {
            return Err(refused());
        }
        for prior in &declarations.entries[..position] {
            check(budget, cancelled)?;
            if prior.number == declaration.number {
                return Err(refused());
            }
        }

        let mut found = false;
        for field in schema.columns() {
            check(budget, cancelled)?;
            if field.number() != declaration.number {
                continue;
            }
            if !matches!(field.native_type(), LONG_TEXT | OLE) || found {
                return Err(refused());
            }
            found = true;
        }
        if !found {
            return Err(refused());
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

#[cfg(test)]
mod tests;
