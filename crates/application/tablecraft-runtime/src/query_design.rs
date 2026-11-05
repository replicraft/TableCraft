//! Explicit metadata-only orchestration of the headless query-design owner.
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
};
use tablecraft_ace::inspect_table_schema;
use tablecraft_builders::query::{
    self, BuiltQueryDesign, DesignFieldMetadata, DesignTableMetadata, MAX_DESIGN_SOURCES,
    QueryDesignDraft,
};
use tablecraft_core::limits::ResourceBudget;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryDesignTarget {
    pub document_revision: u64,
}

impl Document {
    fn design_table(
        &mut self,
        table: &str,
        native: &mut ResourceBudget,
        output: &mut Presentation<'_>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DesignTableMetadata, Error> {
        check(cancelled)?;
        let schema = inspect_table_schema(&mut self.source, SCOPE, table, native, cancelled)?;
        output.allocation(schema.columns().len(), size_of::<DesignFieldMetadata>())?;
        let mut fields = Vec::new();
        fields.try_reserve_exact(schema.columns().len())?;
        for field in schema.columns() {
            fields.push(DesignFieldMetadata {
                name: output.text(field.name())?,
                scalar_type: if field.complex_type_id().is_some() {
                    None
                } else {
                    field.scalar_type()
                },
            });
        }
        let metadata = DesignTableMetadata {
            table: output.text(schema.name())?,
            fields,
        };
        query::validate_metadata(std::slice::from_ref(&metadata))?;
        Ok(metadata)
    }

    pub(crate) fn describe_query_design(
        &mut self,
        target: &QueryDesignTarget,
        table: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if target.document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        let metadata = self.design_table(
            table,
            &mut budget(),
            &mut Presentation::new(cancelled),
            cancelled,
        )?;
        Ok(Response::QueryDesignTable {
            target: target.clone(),
            metadata,
        })
    }

    fn design_metadata(
        &mut self,
        draft: &QueryDesignDraft,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<DesignTableMetadata>, Error> {
        if draft.sources.len() > MAX_DESIGN_SOURCES {
            return Err(query::QueryDesignError::Limit.into());
        }
        query::draft_text_bytes(draft)?;
        let mut native = budget();
        let mut output = Presentation::new(cancelled);
        output.allocation(draft.sources.len(), size_of::<DesignTableMetadata>())?;
        let mut tables: Vec<DesignTableMetadata> = Vec::new();
        tables.try_reserve_exact(draft.sources.len())?;
        for source in &draft.sources {
            check(cancelled)?;
            if tables
                .iter()
                .any(|table| table.table.eq_ignore_ascii_case(&source.table))
            {
                continue;
            }
            tables.push(self.design_table(&source.table, &mut native, &mut output, cancelled)?);
        }
        Ok(tables)
    }

    pub(crate) fn build_query_design(
        &mut self,
        target: &QueryDesignTarget,
        draft: &QueryDesignDraft,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if target.document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        let metadata = self.design_metadata(draft, cancelled)?;
        let built = query::build(draft, &metadata)?;
        check(cancelled)?;
        Ok(design_response(target, built, metadata))
    }

    pub(crate) fn import_query_design(
        &mut self,
        target: &QueryDesignTarget,
        sql: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if target.document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        // Refuse unsupported SQL before even inspecting native source metadata.
        let draft = query::import_unbound(sql)?;
        let metadata = self.design_metadata(&draft, cancelled)?;
        let built = query::import(sql, &metadata)?;
        check(cancelled)?;
        Ok(design_response(target, built, metadata))
    }
}

fn design_response(
    target: &QueryDesignTarget,
    built: BuiltQueryDesign,
    metadata: Vec<DesignTableMetadata>,
) -> Response {
    Response::QueryDesignBuilt {
        target: target.clone(),
        built,
        metadata,
    }
}
