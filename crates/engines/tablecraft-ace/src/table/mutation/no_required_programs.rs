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
    table::mutation::ordinary_content::definition::{
        self, RowDeclaration, tail::RetainedDefinitionTail,
    },
};
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Declaration {
    DenseIntegral,
    DenseRetained,
    Actions,
    SparseGenerated,
    #[cfg(feature = "ace16-create")]
    CurrentSchema,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Rows,
    #[cfg(feature = "ace16-create")]
    HistoricalFixedRows,
    #[cfg(feature = "ace16-create")]
    CurrentSchema,
    #[cfg(feature = "ace16-create")]
    RetainedSchema,
    #[cfg(feature = "ace16-create")]
    CurrentRows,
    #[cfg(feature = "ace16-create")]
    CurrentInsert,
    #[cfg(feature = "ace16-create")]
    RetainedInsert,
    #[cfg(feature = "ace16-create")]
    RetainedRows,
    #[cfg(feature = "ace16-create")]
    RetainedOpaqueRows,
    #[cfg(feature = "ace16-create")]
    RetainedOpaqueInsert,
    #[cfg(feature = "ace16-create")]
    RetainedDelete,
    #[cfg(feature = "ace16-create")]
    CurrentDelete,
}

/// No public constructor or serialized permission. This certificate binds the
/// actual image and strict target catalog row, never an empty program selection.
pub(in crate::table::mutation) struct NoRequiredTablePrograms<'image> {
    image: &'image NativeImage,
    object: ObjectId,
    definition: u32,
    catalog_reference: u32,
    declaration: Declaration,
    purpose: Purpose,
    definition_tail: Option<RetainedDefinitionTail>,
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
        if self.purpose != Purpose::Rows || self.declaration != Declaration::SparseGenerated {
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
        if self.purpose != Purpose::Rows {
            return Err(EditError::Preservation);
        }
        match self.declaration {
            Declaration::DenseIntegral => Ok(false),
            Declaration::DenseRetained => Ok(true),
            Declaration::Actions | Declaration::SparseGenerated => Err(EditError::Preservation),
            #[cfg(feature = "ace16-create")]
            Declaration::CurrentSchema => Err(EditError::Preservation),
        }
    }

    fn capture_with_declaration(
        image: &'image NativeImage,
        schema: &TableSchema,
        declaration: Declaration,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_purpose(image, schema, declaration, Purpose::Rows, budget, cancelled)
    }

    /// Explicit current-schema evidence permits retired ordinals and inert
    /// dictionary names while preserving all program and ownership checks.
    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn capture_current_schema(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_purpose(
            image,
            schema,
            Declaration::CurrentSchema,
            Purpose::CurrentSchema,
            budget,
            cancelled,
        )
    }

    /// Retained schema observations retain a distinct image/target purpose;
    /// row-operation certificates cannot grant metadata mutation authority.
    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn capture_retained_schema(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_purpose(
            image,
            schema,
            Declaration::CurrentSchema,
            Purpose::RetainedSchema,
            budget,
            cancelled,
        )
    }

    /// Current DELETE proves program absence under a distinct source purpose.
    /// Readonly schema and other mutation certificates are not interchangeable.
    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn capture_current_delete(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_purpose(
            image,
            schema,
            Declaration::CurrentSchema,
            Purpose::CurrentDelete,
            budget,
            cancelled,
        )
    }

    /// Current INSERT retains its own source-bound absence purpose. Neither
    /// schema observation nor current-row replacement grants this authority.
    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn capture_current_insert(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_purpose(
            image,
            schema,
            Declaration::CurrentSchema,
            Purpose::CurrentInsert,
            budget,
            cancelled,
        )
    }

    /// Retained surroundings need their own complete ownership proof. Current
    /// target declarations remain separately bound to this insertion purpose;
    /// schema observations alone never grant row-mutation permission.
    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn capture_retained_insert(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_purpose(
            image,
            schema,
            Declaration::CurrentSchema,
            Purpose::RetainedInsert,
            budget,
            cancelled,
        )
    }

    /// Retained row replacement proves its own target programs and complete
    /// storage inventory. Insertion and schema certificates are not transferable.
    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn capture_retained_rows(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_purpose(
            image,
            schema,
            Declaration::CurrentSchema,
            Purpose::RetainedRows,
            budget,
            cancelled,
        )
    }

    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn capture_retained_opaque_rows(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_purpose(
            image,
            schema,
            Declaration::CurrentSchema,
            Purpose::RetainedOpaqueRows,
            budget,
            cancelled,
        )
    }

    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn capture_retained_opaque_insert(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_purpose(
            image,
            schema,
            Declaration::CurrentSchema,
            Purpose::RetainedOpaqueInsert,
            budget,
            cancelled,
        )
    }

    /// Retained deletion proves its own target programs and complete
    /// storage inventory. Insertion, replacement and schema certificates are not transferable.
    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn capture_retained_delete(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_purpose(
            image,
            schema,
            Declaration::CurrentSchema,
            Purpose::RetainedDelete,
            budget,
            cancelled,
        )
    }

    /// Distinct current-row evidence shares checked sparse metadata framing,
    /// but cannot be substituted by a readonly schema certificate.
    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn capture_current_rows(
        image: &'image NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_purpose(
            image,
            schema,
            Declaration::CurrentSchema,
            Purpose::CurrentRows,
            budget,
            cancelled,
        )
    }

    /// Dedicated fixed-row UPDATE proof after closed current-source census.
    /// Schema readonly certificates cannot enter this purpose or its consumer.
    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn capture_historical_fixed_rows(
        image: &'image NativeImage,
        current: &crate::table::mutation::current_schema::Table,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with_purpose(
            image,
            &current.schema,
            Declaration::Actions,
            Purpose::HistoricalFixedRows,
            budget,
            cancelled,
        )
    }

    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn require_historical_fixed_rows_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.purpose != Purpose::HistoricalFixedRows {
            return Err(EditError::Preservation);
        }
        self.require_binding(image, schema, budget, cancelled)
    }

    fn capture_with_purpose(
        image: &'image NativeImage,
        schema: &TableSchema,
        declaration: Declaration,
        purpose: Purpose,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let _span = tracing::debug_span!("native.table.no_required_programs.capture").entered();
        check(budget, cancelled)?;
        match purpose {
            #[cfg(feature = "ace16-create")]
            Purpose::RetainedInsert
            | Purpose::RetainedRows
            | Purpose::RetainedOpaqueRows
            | Purpose::RetainedOpaqueInsert
            | Purpose::RetainedDelete
            | Purpose::RetainedSchema => image.require_retained_capture()?,
            _ => image.require_standard_capture()?,
        }
        let header = image.catalog().header();
        if !matches!(header.format(), FileFormat::Ace2016 | FileFormat::Ace2019)
            || header.subversion() != 0
            || header.encoded_pages()
        {
            return Err(refused());
        }
        let selected = match purpose {
            #[cfg(feature = "ace16-create")]
            Purpose::RetainedRows
            | Purpose::RetainedOpaqueRows
            | Purpose::RetainedOpaqueInsert
            | Purpose::RetainedDelete
            | Purpose::RetainedInsert => {
                let inventory = crate::table::table_append::Inventory::capture_represented(
                    image, budget, cancelled,
                )?;
                catalog::validate_retained(image, schema, &inventory, budget, cancelled)?
            }
            #[cfg(feature = "ace16-create")]
            Purpose::RetainedSchema => {
                let inventory =
                    crate::table::table_append::Inventory::capture(image, budget, cancelled)?;
                catalog::validate_retained(image, schema, &inventory, budget, cancelled)?
            }
            Purpose::Rows => catalog::validate(image, schema, budget, cancelled)?,
            #[cfg(feature = "ace16-create")]
            Purpose::HistoricalFixedRows
            | Purpose::CurrentSchema
            | Purpose::CurrentRows
            | Purpose::CurrentInsert
            | Purpose::CurrentDelete => catalog::validate_schema(image, schema, budget, cancelled)?,
        };
        if selected.properties_present {
            #[cfg(feature = "ace16-create")]
            if matches!(
                purpose,
                Purpose::CurrentSchema
                    | Purpose::RetainedSchema
                    | Purpose::CurrentRows
                    | Purpose::CurrentInsert
                    | Purpose::RetainedInsert
                    | Purpose::RetainedRows
                    | Purpose::RetainedOpaqueRows
                    | Purpose::RetainedOpaqueInsert
                    | Purpose::RetainedDelete
                    | Purpose::CurrentDelete
            ) {
                properties::capture_current_schema(image, schema, budget, cancelled)?;
            } else {
                properties::capture(image, schema, budget, cancelled)?;
            }
            #[cfg(not(feature = "ace16-create"))]
            properties::capture(image, schema, budget, cancelled)?;
        }
        let catalog_reference = selected.reference;
        declarations::capture(image, budget, cancelled)?;
        let row_declaration = match declaration {
            Declaration::DenseIntegral => RowDeclaration::DenseIntegral,
            Declaration::DenseRetained => RowDeclaration::DenseRetained,
            Declaration::Actions => RowDeclaration::Actions,
            Declaration::SparseGenerated => RowDeclaration::SparseGenerated,
            #[cfg(feature = "ace16-create")]
            Declaration::CurrentSchema => RowDeclaration::CurrentSchema,
        };
        let definition_tail =
            definition::validate_rows(image, schema, row_declaration, budget, cancelled)?;

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
            purpose,
            definition_tail,
        })
    }

    pub(in crate::table::mutation) fn share_definition_tail(
        &self,
    ) -> Option<RetainedDefinitionTail> {
        self.definition_tail
            .as_ref()
            .map(RetainedDefinitionTail::share)
    }

    pub(in crate::table::mutation) fn require_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.purpose != Purpose::Rows {
            return Err(EditError::Preservation);
        }
        self.require_binding(image, schema, budget, cancelled)
    }

    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn require_current_schema_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.purpose != Purpose::CurrentSchema {
            return Err(EditError::Preservation);
        }
        self.require_binding(image, schema, budget, cancelled)
    }

    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn require_retained_schema_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.purpose != Purpose::RetainedSchema || self.declaration != Declaration::CurrentSchema
        {
            return Err(EditError::Preservation);
        }
        self.require_binding(image, schema, budget, cancelled)
    }

    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn require_current_rows_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.purpose != Purpose::CurrentRows {
            return Err(EditError::Preservation);
        }
        self.require_binding(image, schema, budget, cancelled)
    }

    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn require_current_insert_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.purpose != Purpose::CurrentInsert {
            return Err(EditError::Preservation);
        }
        self.require_binding(image, schema, budget, cancelled)
    }

    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn require_retained_rows_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.purpose != Purpose::RetainedRows || self.declaration != Declaration::CurrentSchema {
            return Err(EditError::Preservation);
        }
        self.require_binding(image, schema, budget, cancelled)
    }

    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn require_retained_opaque_rows_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.purpose != Purpose::RetainedOpaqueRows
            || self.declaration != Declaration::CurrentSchema
        {
            return Err(EditError::Preservation);
        }
        self.require_binding(image, schema, budget, cancelled)
    }

    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn require_retained_opaque_insert_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.purpose != Purpose::RetainedOpaqueInsert
            || self.declaration != Declaration::CurrentSchema
        {
            return Err(EditError::Preservation);
        }
        self.require_binding(image, schema, budget, cancelled)
    }

    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn require_retained_delete_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.purpose != Purpose::RetainedDelete || self.declaration != Declaration::CurrentSchema
        {
            return Err(EditError::Preservation);
        }
        self.require_binding(image, schema, budget, cancelled)
    }

    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn require_retained_insert_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.purpose != Purpose::RetainedInsert || self.declaration != Declaration::CurrentSchema
        {
            return Err(EditError::Preservation);
        }
        self.require_binding(image, schema, budget, cancelled)
    }

    #[cfg(feature = "ace16-create")]
    pub(in crate::table::mutation) fn require_current_delete_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.purpose != Purpose::CurrentDelete {
            return Err(EditError::Preservation);
        }
        self.require_binding(image, schema, budget, cancelled)
    }

    fn require_binding(
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

#[cfg(all(test, feature = "ace16-create"))]
mod schema_tests;

#[cfg(all(test, feature = "ace16-create"))]
mod current_insert_tests;

#[cfg(all(test, feature = "ace16-create"))]
mod current_delete_tests;

#[cfg(all(test, feature = "ace16-create"))]
mod retained_insert_tests;

#[cfg(all(test, feature = "ace16-create"))]
mod represented_catalog_tests;
