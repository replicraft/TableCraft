use super::Mode;

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Explicit MDB/ACCDB file path.
    path: std::path::PathBuf,
    /// Decode bounded object metadata; never executes or extracts object bodies.
    #[arg(long)]
    catalog: bool,
    /// Include system tables in catalog output; other object kinds are always visible.
    #[arg(long, requires = "catalog")]
    show_system: bool,
    /// Inspect one saved query's inert native fragments without executing SQL.
    #[arg(
        long,
        value_name = "NAME",
        conflicts_with_all = ["catalog", "relationships", "table", "indexes", "properties",
            "features", "complex", "stored_calculation", "complex_links", "check", "constraints"]
    )]
    query: Option<String>,
    /// Inspect inert relationship declarations, including composite mappings.
    #[arg(long, conflicts_with_all = ["catalog", "table", "indexes", "properties", "check", "constraints"])]
    relationships: bool,
    /// Select a local table for schema inspection or explicit checking.
    #[arg(long, value_name = "NAME", conflicts_with = "catalog")]
    table: Option<String>,
    /// Inspect native index metadata for the selected table.
    #[arg(long, requires = "table")]
    indexes: bool,
    /// Inspect inert native table/column/index properties without evaluating them.
    #[arg(long, requires = "table", conflicts_with_all = ["indexes", "check"])]
    properties: bool,
    /// Inspect inert calculated, complex and presentation field declarations.
    #[arg(long, requires = "table", conflicts_with_all = ["indexes", "properties", "check", "constraints", "relationships"])]
    features: bool,
    /// Resolve native complex fields to their type and child-table schemas.
    #[arg(long, requires = "table", conflicts_with_all = ["indexes", "properties", "features", "check", "constraints", "relationships"])]
    complex: bool,
    /// Inspect qualified stored calculation values without recalculating them.
    #[arg(long, value_name = "COLUMN", requires = "table", conflicts_with_all = ["indexes", "properties", "features", "complex", "check", "constraints", "relationships"])]
    stored_calculation: Option<String>,
    /// Inspect scoped complex parent keys and raw child memberships.
    #[arg(long, value_name="COLUMN", requires="table", conflicts_with_all=["indexes","properties","features","complex","stored_calculation","check","constraints","relationships"])]
    complex_links: Option<String>,
    /// Inspect stored attachment metadata; never decodes, extracts or opens files/URLs.
    #[arg(long, value_name="COLUMN", requires="table", conflicts_with_all=["query","relationships","indexes","properties","features","complex","stored_calculation","complex_links","version_history","check","constraints"])]
    attachments: Option<String>,
    /// Inspect stored Memo versions in snapshot order, without selecting a latest value.
    #[arg(long, value_name="COLUMN", requires="table", conflicts_with_all=["query","relationships","indexes","properties","features","complex","stored_calculation","complex_links","attachments","check","constraints"])]
    version_history: Option<String>,
    /// Read all scalar rows and qualified indexes; report unchecked profiles.
    #[arg(long, requires = "table", conflicts_with = "indexes")]
    check: bool,
    /// Explicitly audit field declarations and qualified foreign-key records.
    #[arg(long, requires = "table", conflicts_with_all = ["indexes", "properties", "check"])]
    constraints: bool,
}

impl Args {
    pub(crate) fn run(
        self,
        format: crate::presentation::Format,
        password: Option<&str>,
    ) -> Result<String, super::Error> {
        let Self {
            path,
            catalog,
            show_system,
            query,
            relationships,
            table,
            indexes,
            properties,
            features,
            complex,
            stored_calculation,
            complex_links,
            attachments,
            version_history,
            check,
            constraints,
        } = self;
        let mode = if let Some(name) = query.as_deref() {
            Mode::SavedQuery(name)
        } else if relationships {
            Mode::Relationships
        } else if let Some(name) = table.as_deref() {
            if let Some(column) = attachments.as_deref() {
                Mode::Attachments(name, column)
            } else if let Some(column) = version_history.as_deref() {
                Mode::VersionHistory(name, column)
            } else if let Some(column) = complex_links.as_deref() {
                Mode::ComplexLinks(name, column)
            } else if let Some(column) = stored_calculation.as_deref() {
                Mode::StoredCalculation(name, column)
            } else if complex {
                Mode::Complex(name)
            } else if features {
                Mode::Features(name)
            } else if check {
                Mode::Check(name)
            } else if constraints {
                Mode::Constraints(name)
            } else if properties {
                Mode::Properties(name)
            } else if indexes {
                Mode::Indexes(name)
            } else {
                Mode::Table(name)
            }
        } else if catalog {
            Mode::Catalog { show_system }
        } else {
            Mode::Header
        };
        super::run(&path, mode, format, password)
    }
}
