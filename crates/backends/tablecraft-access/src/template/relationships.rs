//! OPC control parts select inert template content; no external URI is followed.
use super::{
    TEMPLATE_RELATIONSHIP, TemplateError, TemplatePackage, TemplatePart, TemplateRelationship,
    check, name, xml,
};
use std::collections::{BTreeMap, BTreeSet};
use tablecraft_core::limits::ResourceBudget;

const CONTENT_TYPES: &str = "[Content_Types].xml";
const TYPES_NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const RELATIONSHIPS_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const RELATIONSHIP_CONTENT_TYPE: &str = "application/vnd.openxmlformats-package.relationships+xml";

pub(super) fn assemble(
    raw: BTreeMap<String, Vec<u8>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TemplatePackage, TemplateError> {
    let types = raw.get(CONTENT_TYPES).ok_or(TemplateError::Malformed)?;
    // The archive preflight reserves per-entry metadata, including folded keys.
    let mut raw_keys = BTreeSet::new();
    for name in raw.keys() {
        check(budget, cancelled)?;
        raw_keys.insert(name.to_ascii_lowercase());
    }
    let mut defaults = BTreeMap::new();
    let mut overrides = BTreeMap::new();
    for mut record in xml::records(types, TYPES_NS, "Types", budget, cancelled)? {
        let content_type = take(&mut record.attributes, "ContentType")?;
        if !content_type.contains('/') || content_type.chars().any(char::is_control) {
            return Err(TemplateError::Malformed);
        }
        match record.name.as_str() {
            "Default" => {
                let extension = take(&mut record.attributes, "Extension")?.to_ascii_lowercase();
                if extension.is_empty()
                    || extension.contains(['/', '.', '\\'])
                    || defaults.insert(extension, content_type).is_some()
                {
                    return Err(TemplateError::Malformed);
                }
            }
            "Override" => {
                let part = take(&mut record.attributes, "PartName")?;
                let part = part.strip_prefix('/').ok_or(TemplateError::Malformed)?;
                name(part)?;
                let key = part.to_ascii_lowercase();
                if !raw_keys.contains(&key) || overrides.insert(key, content_type).is_some() {
                    return Err(TemplateError::Malformed);
                }
            }
            _ => return Err(TemplateError::Unsupported),
        }
        if !record.attributes.is_empty() {
            return Err(TemplateError::Unsupported);
        }
    }
    drop(raw_keys);
    let mut parts = BTreeMap::new();
    let mut controls = Vec::new();
    for (part_name, bytes) in raw {
        check(budget, cancelled)?;
        if part_name == CONTENT_TYPES {
            continue;
        }
        let key = part_name.to_ascii_lowercase();
        let content_type = overrides
            .remove(&key)
            .or_else(|| {
                let (_, extension) = part_name.rsplit_once('.')?;
                defaults.get(&extension.to_ascii_lowercase()).cloned()
            })
            .ok_or(TemplateError::Malformed)?;
        if let Some(source) = relationship_source(&key)? {
            if !content_type.eq_ignore_ascii_case(RELATIONSHIP_CONTENT_TYPE) {
                return Err(TemplateError::Malformed);
            }
            controls.push((source, bytes));
        } else {
            parts.insert(
                key,
                TemplatePart {
                    name: part_name,
                    content_type,
                    bytes,
                    relationships: Vec::new(),
                },
            );
        }
    }
    let mut root_relationships = None;
    for (source, bytes) in controls {
        let mut relationships = Vec::new();
        let mut ids = BTreeSet::new();
        for mut record in
            xml::records(&bytes, RELATIONSHIPS_NS, "Relationships", budget, cancelled)?
        {
            if record.name != "Relationship" {
                return Err(TemplateError::Unsupported);
            }
            let id = take(&mut record.attributes, "Id")?;
            let kind = take(&mut record.attributes, "Type")?;
            let target = take(&mut record.attributes, "Target")?;
            if !id.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_alphabetic()
                    || byte == b'_'
                    || (index > 0 && (byte.is_ascii_digit() || matches!(byte, b'-' | b'.')))
            }) || !kind.contains(':')
                || kind.chars().any(|c| c.is_control() || c.is_whitespace())
            {
                return Err(TemplateError::Unsupported);
            }
            if record
                .attributes
                .remove("TargetMode")
                .is_some_and(|mode| mode != "Internal")
            {
                return Err(TemplateError::Unsupported);
            }
            if !record.attributes.is_empty() || !ids.insert(id.clone()) {
                return Err(TemplateError::Malformed);
            }
            let target = resolve(&source, &target)?;
            let target = parts
                .get(&target.to_ascii_lowercase())
                .ok_or(TemplateError::Malformed)?
                .name
                .clone();
            relationships.push(TemplateRelationship { id, kind, target });
        }
        if source.is_empty() {
            if root_relationships.replace(relationships).is_some() {
                return Err(TemplateError::Malformed);
            }
        } else {
            parts
                .get_mut(&source)
                .ok_or(TemplateError::Malformed)?
                .relationships = relationships;
        }
    }
    let root_relationships = root_relationships.ok_or(TemplateError::Malformed)?;
    let metadata: Vec<_> = root_relationships
        .iter()
        .filter(|relationship| relationship.kind == TEMPLATE_RELATIONSHIP)
        .collect();
    if metadata.len() != 1 {
        return Err(TemplateError::Malformed);
    }
    let metadata = metadata[0].target.to_ascii_lowercase();
    if !parts[&metadata]
        .content_type
        .eq_ignore_ascii_case("text/xml")
    {
        return Err(TemplateError::Malformed);
    }
    check(budget, cancelled)?;
    Ok(TemplatePackage {
        parts,
        root_relationships,
        metadata,
    })
}

fn take(attributes: &mut BTreeMap<String, String>, key: &str) -> Result<String, TemplateError> {
    attributes
        .remove(key)
        .filter(|value| !value.is_empty())
        .ok_or(TemplateError::Malformed)
}

fn relationship_source(part: &str) -> Result<Option<String>, TemplateError> {
    if part == "_rels/.rels" {
        return Ok(Some(String::new()));
    }
    if let Some((directory, leaf)) = part.rsplit_once("/_rels/") {
        let leaf = leaf.strip_suffix(".rels").ok_or(TemplateError::Malformed)?;
        return Ok(Some(format!("{directory}/{leaf}")));
    }
    if let Some(leaf) = part.strip_prefix("_rels/") {
        return Ok(Some(
            leaf.strip_suffix(".rels")
                .ok_or(TemplateError::Malformed)?
                .into(),
        ));
    }
    Ok(None)
}

fn resolve(source: &str, target: &str) -> Result<String, TemplateError> {
    let mut components: Vec<_> = if target.starts_with('/') {
        Vec::new()
    } else {
        source
            .rsplit_once('/')
            .map_or(Vec::new(), |(directory, _)| directory.split('/').collect())
    };
    for component in target.strip_prefix('/').unwrap_or(target).split('/') {
        match component {
            "." => {}
            ".." => {
                components.pop().ok_or(TemplateError::Unsupported)?;
            }
            "" => return Err(TemplateError::Unsupported),
            component => components.push(component),
        }
    }
    let resolved = components.join("/");
    name(&resolved)?;
    Ok(resolved)
}
