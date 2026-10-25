//! Inert ACCDT package inventory; schema/data interpretation is a separate layer.
mod archive;
mod encoding;
mod objects;
mod part_uri;
mod relationships;
mod tree;
mod xml;
mod xml_names;
pub use objects::TemplateObject;
mod data;
pub use data::TemplateTable;
mod source;
pub use source::TemplateSource;

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

const TEMPLATE_RELATIONSHIP: &str =
    "http://schemas.microsoft.com/office/access/2005/04/template/start";
const MAX_PARTS: usize = 4096;
const MAX_NAME_BYTES: usize = 1024;
const MAX_PACKAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_EXPANDED_BYTES: usize = 64 * 1024 * 1024;

/// Template failures omit package content, part names and dependency diagnostics.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TemplateError {
    /// I/O, cancellation, budget or allocation failure.
    #[error(transparent)]
    Read(#[from] InspectError),
    /// Inconsistent archive, XML, content types or relationship graph.
    #[error("malformed Access template package")]
    Malformed,
    /// Unsupported ZIP/OPC feature, external target or resource ceiling.
    #[error("Access template package feature or size is not supported")]
    Unsupported,
}

/// One validated internal OPC relationship. Targets are package-relative names.
#[derive(Debug)]
pub struct TemplateRelationship {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) target: String,
}

impl TemplateRelationship {
    /// Identifier unique within the relationship's source part.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Declared relationship type URI; it grants no execution permission.
    pub fn kind(&self) -> &str {
        &self.kind
    }
    /// Resolved logical IRI part name, without a leading slash.
    pub fn target(&self) -> &str {
        &self.target
    }
}

/// Immutable, bounded, in-memory template part. Bytes remain inert.
pub struct TemplatePart {
    name: String,
    content_type: String,
    bytes: Vec<u8>,
    relationships: Vec<TemplateRelationship>,
}

impl TemplatePart {
    /// Validated package-relative logical IRI name, after ZIP UTF-8 mapping.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// OPC content type resolved from the content-types part.
    pub fn content_type(&self) -> &str {
        &self.content_type
    }
    /// Stored/decompressed content, with checked ZIP CRC. No code is run.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Internal relationships from this part, validated against the package.
    pub fn relationships(&self) -> &[TemplateRelationship] {
        &self.relationships
    }
}

/// Validated ZIP/OPC inventory with one template metadata relationship.
/// This does not instantiate a database, interpret schemas or verify signatures.
/// Sources must remain stable/offline while opening. No disk extraction occurs.
pub struct TemplatePackage {
    parts: BTreeMap<String, TemplatePart>,
    root_relationships: Vec<TemplateRelationship>,
    metadata: String,
}

impl TemplatePackage {
    /// Select ZIP template input by content. Non-ZIP sources are rewound and
    /// returned as None; malformed/non-template ZIP files fail without fallback.
    pub fn open_if_template<R: Read + Seek>(
        source: &mut R,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<Self>, TemplateError> {
        const PREFIX_BYTES: usize = size_of::<u32>();
        const SIGNATURES: [[u8; PREFIX_BYTES]; 3] =
            [*b"PK\x03\x04", *b"PK\x05\x06", *b"PK\x07\x08"];
        budget
            .charge(Resource::Bytes, PREFIX_BYTES as u64)
            .map_err(InspectError::from)?;
        let mut prefix = [0; PREFIX_BYTES];
        tablecraft_io::read_exact_at(source, 0, &mut prefix, budget, cancelled)
            .map_err(InspectError::from)?;
        if SIGNATURES.contains(&prefix) {
            return Self::open(source, budget, cancelled).map(Some);
        }
        source
            .seek(SeekFrom::Start(0))
            .map_err(InspectError::from)?;
        Ok(None)
    }

    /// Capture a classic single-disk stored/deflated OPC package under shared
    /// budgets and cancellation. ZIP64, encryption and external targets fail.
    pub fn open<R: Read + Seek>(
        source: &mut R,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, TemplateError> {
        let _span = tracing::debug_span!("access.template.open").entered();
        let parts = archive::read(source, budget, cancelled)?;
        relationships::assemble(parts, budget, cancelled)
    }

    /// All content parts in ASCII-folded package-name order, excluding OPC controls.
    pub fn parts(&self) -> impl ExactSizeIterator<Item = &TemplatePart> {
        self.parts.values()
    }
    /// OPC ASCII-case-insensitive lookup, without Unicode case folding.
    /// Names are package-relative identifiers, never filesystem paths.
    pub fn part(&self, name: &str) -> Option<&TemplatePart> {
        if name.len() > MAX_NAME_BYTES {
            return None;
        }
        // Bound lookup scratch without a heap allocation or caller-owned budget.
        let mut key = [0; MAX_NAME_BYTES];
        let key = &mut key[..name.len()];
        key.copy_from_slice(name.as_bytes());
        key.make_ascii_lowercase();
        // ASCII folding preserves UTF-8 encoding.
        self.parts.get(std::str::from_utf8(key).ok()?)
    }
    /// The unique template metadata part selected by the root relationship.
    pub fn metadata(&self) -> &TemplatePart {
        // Construction validates existence before publishing the package.
        &self.parts[&self.metadata]
    }
    /// Validated package-level internal relationships.
    pub fn relationships(&self) -> &[TemplateRelationship] {
        &self.root_relationships
    }
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), TemplateError> {
    if cancelled() {
        return Err(InspectError::Cancelled.into());
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)?;
    Ok(())
}

fn allocate(size: usize, budget: &mut ResourceBudget) -> Result<Vec<u8>, TemplateError> {
    budget
        .charge(Resource::Bytes, size as u64)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| InspectError::Allocation)?;
    bytes.resize(size, 0);
    Ok(bytes)
}

#[cfg(test)]
mod tests;
