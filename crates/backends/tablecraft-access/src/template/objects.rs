//! Object identities come from metadata relationships, never filenames or bodies.
use super::{TemplateError, TemplatePackage, TemplatePart, check, tree};
use std::collections::BTreeSet;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::ObjectKind;
use tablecraft_schema::SchemaName;

const OBJECT_NS: &str =
    "http://schemas.microsoft.com/office/access/2005/04/template/object-metadata";
const OBJECT_PREFIX: &str = "http://schemas.microsoft.com/office/access/2005/04/template/";
const OFFICE_PREFIX: &str = "http://schemas.microsoft.com/office/2007/relationships/";
const LIST_RELATIONSHIP: &str =
    "http://schemas.microsoft.com/office/2007/relationships/ListInstanceDefinition";
const METADATA_RELATIONSHIP: &str =
    "http://schemas.microsoft.com/office/access/2005/04/template/object-metadata";
const DATA_RELATIONSHIP: &str =
    "http://schemas.microsoft.com/office/access/2005/04/template/table-data";

/// Inert template object identity with source-specific part references.
/// This does not validate or instantiate the object's application body.
pub struct TemplateObject {
    name: SchemaName,
    kind: ObjectKind,
    declared_type: String,
    linked: bool,
    list_definition: bool,
    part: String,
    metadata: String,
    data: Option<String>,
}
impl TemplateObject {
    /// Validated user-facing Access name from object metadata.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    /// Shared object category; linked tables retain a separate storage marker.
    pub fn kind(&self) -> ObjectKind {
        self.kind
    }
    /// Original metadata type spelling, including unknown future categories.
    pub fn declared_type(&self) -> &str {
        &self.declared_type
    }
    /// Whether this object declares external linked storage; never followed.
    pub fn is_linked(&self) -> bool {
        self.linked
    }
    /// Whether the object uses a SharePoint List Definition part. Its SOAP
    /// construction/schema agreement is not supplied by the inline-data reader.
    pub fn is_list_definition(&self) -> bool {
        self.list_definition
    }
    /// Package-relative object/list definition part.
    pub fn part(&self) -> &str {
        &self.part
    }
    /// Package-relative metadata part that supplies name and category.
    pub fn metadata_part(&self) -> &str {
        &self.metadata
    }
    /// Optional explicitly related stored table-data part; absence is not proof
    /// of an empty table and does not authorize external retrieval.
    pub fn data_part(&self) -> Option<&str> {
        self.data.as_deref()
    }
}

impl TemplatePackage {
    /// Read bounded object identities from template metadata relationships.
    /// Table records and application bodies are not interpreted. Unknown internal
    /// relationship types remain in the package inventory rather than executing.
    pub fn objects(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<TemplateObject>, TemplateError> {
        let _span = tracing::debug_span!("access.template.objects").entered();
        let mut objects = Vec::new();
        let mut names = BTreeSet::new();
        let mut targets = BTreeSet::new();
        for relationship in self.metadata().relationships() {
            check(budget, cancelled)?;
            let relation = if relationship.kind == LIST_RELATIONSHIP {
                "table"
            } else if relationship.kind == format!("{OBJECT_PREFIX}object") {
                "object"
            } else {
                match relationship.kind.strip_prefix(OFFICE_PREFIX) {
                    Some("Link") => "linked-table",
                    Some("Query") => "query",
                    Some("Form") => "form",
                    Some("Report") => "report",
                    Some("Macro") => "macro",
                    _ => continue,
                }
            };
            if !targets.insert(relationship.target.as_str()) {
                return Err(TemplateError::Malformed);
            }
            let part = self
                .part(&relationship.target)
                .ok_or(TemplateError::Malformed)?;
            let metadata =
                selected(self, part, METADATA_RELATIONSHIP)?.ok_or(TemplateError::Malformed)?;
            if !metadata
                .content_type()
                .eq_ignore_ascii_case("application/xml")
            {
                return Err(TemplateError::Malformed);
            }
            let (name, kind, linked, type_name) = identity(metadata.bytes(), budget, cancelled)?;
            if kind != ObjectKind::Unknown
                && (relationship.kind == LIST_RELATIONSHIP && (kind != ObjectKind::Table || linked)
                    || relation == "linked-table" && !linked
                    || relation == "query" && kind != ObjectKind::Query
                    || relation == "form" && kind != ObjectKind::Form
                    || relation == "report" && kind != ObjectKind::Report
                    || relation == "macro" && kind != ObjectKind::Macro)
            {
                return Err(TemplateError::Malformed);
            }
            let identity = (
                if linked {
                    "Table".to_owned()
                } else {
                    type_name.clone()
                },
                name.as_str().to_ascii_lowercase(),
            );
            if !names.insert(identity) {
                return Err(TemplateError::Malformed);
            }
            let data = selected(self, part, DATA_RELATIONSHIP)?;
            if data.is_some_and(|data| !data.content_type().eq_ignore_ascii_case("application/xml"))
                || data.is_some()
                    && (linked || !matches!(kind, ObjectKind::Table | ObjectKind::Unknown))
            {
                return Err(TemplateError::Malformed);
            }
            budget
                .charge(
                    Resource::Bytes,
                    (size_of::<TemplateObject>()
                        + part.name().len()
                        + metadata.name().len()
                        + data.map_or(0, |part| part.name().len())) as u64,
                )
                .map_err(InspectError::from)?;
            objects
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            objects.push(TemplateObject {
                name,
                kind,
                declared_type: type_name,
                linked,
                list_definition: relationship.kind == LIST_RELATIONSHIP,
                part: part.name().into(),
                metadata: metadata.name().into(),
                data: data.map(|part| part.name().into()),
            });
        }
        check(budget, cancelled)?;
        Ok(objects)
    }
}

fn selected<'a>(
    package: &'a TemplatePackage,
    part: &TemplatePart,
    kind: &str,
) -> Result<Option<&'a TemplatePart>, TemplateError> {
    let mut relationships = part
        .relationships()
        .iter()
        .filter(|relationship| relationship.kind == kind);
    let result = relationships
        .next()
        .map(|relationship| {
            package
                .part(&relationship.target)
                .ok_or(TemplateError::Malformed)
        })
        .transpose()?;
    if relationships.next().is_some() {
        return Err(TemplateError::Malformed);
    }
    Ok(result)
}

fn identity(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(SchemaName, ObjectKind, bool, String), TemplateError> {
    let root = tree::parse(bytes, budget, cancelled)?;
    root.whitespace()?;
    if !root.is(OBJECT_NS, "AccessObject")
        || !root.attributes.is_empty()
        || !(2..=3).contains(&root.children.len())
        || !root.children[0].is(OBJECT_NS, "Type")
        || !root.children[1].is(OBJECT_NS, "Name")
        || root
            .children
            .get(2)
            .is_some_and(|node| !node.is(OBJECT_NS, "NameMap"))
    {
        return Err(TemplateError::Malformed);
    }
    let type_name = root.children[0].scalar()?;
    if type_name.is_empty()
        || type_name
            .chars()
            .any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(TemplateError::Malformed);
    }
    let (kind, linked) = match type_name {
        "Table" => (ObjectKind::Table, false),
        "Link" | "SQLLink" => (ObjectKind::Table, true),
        "Query" => (ObjectKind::Query, false),
        "Form" => (ObjectKind::Form, false),
        "Report" => (ObjectKind::Report, false),
        "Macro" => (ObjectKind::Macro, false),
        "Module" => (ObjectKind::Module, false),
        _ => (ObjectKind::Unknown, false),
    };
    let name = root.children[1].scalar()?;
    let name = SchemaName::new(name.to_owned()).map_err(|_| TemplateError::Malformed)?;
    Ok((name, kind, linked, type_name.to_owned()))
}
