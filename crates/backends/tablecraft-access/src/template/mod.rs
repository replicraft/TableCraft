//! Inert ACCDT package inventory; schema/data interpretation is a separate layer.
mod archive;
mod relationships;
mod xml;

use std::collections::BTreeMap;
use std::io::{Read, Seek};
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
    /// Resolved internal part name, without a leading slash.
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
    /// Validated package-relative name.
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

    /// All content parts in package-name order, excluding OPC control parts.
    pub fn parts(&self) -> impl ExactSizeIterator<Item = &TemplatePart> {
        self.parts.values()
    }
    /// Exact-name lookup; names come from this package, not filesystem paths.
    pub fn part(&self, name: &str) -> Option<&TemplatePart> {
        self.parts.get(name)
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

fn name(value: &str) -> Result<(), TemplateError> {
    if value.is_empty() || value.len() > MAX_NAME_BYTES {
        return Err(TemplateError::Unsupported);
    }
    if value
        .chars()
        .any(|c| c.is_control() || matches!(c, '\\' | ':' | '%' | '#' | '?'))
        || value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(TemplateError::Unsupported);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
