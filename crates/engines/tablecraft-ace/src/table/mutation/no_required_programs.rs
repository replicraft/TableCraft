//! Source-bound admission for the strict native target program-storage profile.
//! Inferred bounded profile; native Access save/reopen conformance remains pending.
mod catalog;
pub(in crate::table::mutation) mod declarations;
mod properties;

#[cfg(test)]
mod tests;

use crate::{
    EditError, FileFormat, NativeImage, PayloadPresence, TableExtraCoverage, TableSchema,
    WriteBlocker,
    format::data_type::{GUID, SHORT_BINARY, SHORT_TEXT},
    header::check,
    inspect_table_indexes,
};
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Declaration {
    DenseIntegral,
    DenseRetained,
    Actions,
    SparseGenerated,
}

/// No public constructor or serialized permission. This certificate binds the
/// actual image and strict target catalog row, never an empty program selection.
pub(in crate::table::mutation) struct NoRequiredTablePrograms<'image> {
    image: &'image NativeImage,
    object: ObjectId,
    definition: u32,
    catalog_reference: u32,
    declaration: Declaration,
}

impl<'image> NoRequiredTablePrograms<'image> {
    pub(in crate::table::mutation) fn capture(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_declaration(image, schema, Declaration::DenseIntegral, budget, cancelled)
    }

    /// Fresh complete native absence proof with action-only column identities.
    pub(in crate::table::mutation) fn capture_actions(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_declaration(image, schema, Declaration::Actions, budget, cancelled)
    }

    /// Original INSERT metadata selects one complete declaration attempt.
    pub(in crate::table::mutation) fn capture_generated_insert(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_declaration(
            image,
            schema,
            Declaration::SparseGenerated,
            budget,
            cancelled,
        )
    }

    pub(in crate::table::mutation) fn require_generated_insert(&self) -> Result<(), EditError> {
        if self.declaration != Declaration::SparseGenerated {
            return Err(EditError::Preservation);
        }
        Ok(())
    }

    /// Dense native UPDATE declarations retain checked readonly scalar indexes.
    /// This includes manual single-GUID keys under complete membership proof.
    /// This proves no mutation target; each planner still binds its actual fields.
    pub(in crate::table::mutation) fn capture_updates(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        let mut retained = false;
        let mut guid_candidate = false;
        if schema.layout.physical_indexes != 0 {
            for field in schema.columns() {
                check(budget, cancelled)?;
                retained |= matches!(field.native_type(), SHORT_BINARY | SHORT_TEXT);
                guid_candidate |= field.native_type() == GUID;
            }
        }
        if !retained && guid_candidate {
            // Only an actual GUID key selects the new readonly family. An
            // unrelated GUID field keeps the former integral attempt/order.
            let indexes = inspect_table_indexes(
                &mut image.reader(),
                image.catalog().catalog().scope(),
                schema.name(),
                budget,
                cancelled,
            )?;
            if indexes.schema().object_id() != schema.object_id()
                || indexes.schema().definition_page() != schema.definition_page()
            {
                return Err(EditError::Preservation);
            }
            for index in indexes.indexes() {
                check(budget, cancelled)?;
                for key in index.columns() {
                    check(budget, cancelled)?;
                    for field in schema.columns() {
                        check(budget, cancelled)?;
                        if field.number() == key.number() && field.native_type() == GUID {
                            retained = true;
                        }
                    }
                }
            }
        }
        // Metadata chooses a declaration attempt before content admission. Integral-only
        // profiles retain their actual legacy consumer; no failure is retried.
        if !retained {
            return Self::capture(image, schema, budget, cancelled);
        }
        Self::capture_with_declaration(image, schema, Declaration::DenseRetained, budget, cancelled)
    }

    pub(in crate::table::mutation) fn retained_update_keys(&self) -> Result<bool, EditError> {
        match self.declaration {
            Declaration::DenseIntegral => Ok(false),
            Declaration::DenseRetained => Ok(true),
            Declaration::Actions | Declaration::SparseGenerated => Err(EditError::Preservation),
        }
    }

    fn capture_with_declaration(
        image: &'image NativeImage,
        schema: &TableSchema,
        declaration: Declaration,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let _span = tracing::debug_span!("native.table.no_required_programs.capture").entered();
        check(budget, cancelled)?;
        image.require_standard_capture()?;
        let header = image.catalog().header();
        if !matches!(header.format(), FileFormat::Ace2016 | FileFormat::Ace2019)
            || header.subversion() != 0
            || header.encoded_pages()
        {
            return Err(refused());
        }
        let selected = catalog::validate(image, schema, budget, cancelled)?;
        if selected.properties_present {
            properties::capture(image, schema, budget, cancelled)?;
        }
        let catalog_reference = selected.reference;
        declarations::capture(image, budget, cancelled)?;
        match declaration {
            Declaration::DenseIntegral => {
                super::ordinary_content::definition::validate_user(
                    image, schema, budget, cancelled,
                )?;
            }
            Declaration::DenseRetained => {
                super::ordinary_content::definition::validate_retained_updates(
                    image, schema, budget, cancelled,
                )?;
            }
            Declaration::Actions => {
                super::ordinary_content::definition::validate_actions(
                    image, schema, budget, cancelled,
                )?;
            }
            Declaration::SparseGenerated => {
                super::ordinary_content::definition::validate_generated(
                    image, schema, budget, cancelled,
                )?;
            }
        }

        // Public coverage corroborates the raw code-carrier NULL checks,
        // strict property qualification and physical catalog census. Its three
        // slots alone establish neither property completeness nor row closure.
        let coverage = image.inspect_table_macro_coverage(schema.object_id(), budget, cancelled)?;
        if coverage.table_origin().object_id() != schema.object_id()
            || coverage.table_origin().native_id() <= 0
            || coverage.table_origin().native_id() as u32 != schema.definition_page()
            || coverage.body().presence() != PayloadPresence::Absent
            || coverage.module().presence() != PayloadPresence::Absent
            || coverage.table_extra().presence() != PayloadPresence::Absent
            || coverage.table_extra().coverage() != TableExtraCoverage::NotPresent
            || !coverage.children().is_empty()
        {
            return Err(refused());
        }
        check(budget, cancelled)?;
        Ok(Self {
            image,
            object: schema.object_id(),
            definition: schema.definition_page(),
            catalog_reference,
            declaration,
        })
    }

    pub(in crate::table::mutation) fn require_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        if !std::ptr::eq(self.image, image)
            || self.object != schema.object_id()
            || self.definition != schema.definition_page()
        {
            return Err(EditError::Preservation);
        }
        let mut found = false;
        for origin in image.catalog().origins() {
            check(budget, cancelled)?;
            if origin.object_id() == self.object {
                let reference = crate::format::row::reference(origin.page(), origin.slot());
                if found || reference != self.catalog_reference {
                    return Err(EditError::Preservation);
                }
                found = true;
            }
        }
        if !found {
            return Err(EditError::Preservation);
        }
        Ok(())
    }
}

fn refused() -> EditError {
    EditError::Unsupported(WriteBlocker::ActiveContent)
}

#[cfg(test)]
mod resource_tests;

#[cfg(test)]
mod action_tests;

#[cfg(test)]
mod ignore_null_prior_stages;

#[cfg(test)]
mod guid_prior_stages;
