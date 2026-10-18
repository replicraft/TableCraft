use crate::{CatalogScope, ModelError, ObjectDescriptor, ObjectKind};
use std::collections::HashSet;
use tablecraft_core::ids::ObjectId;
use tablecraft_schema::NameComparison;

/// Read-only object inventory. Native decoding and object bodies are external.
pub struct ObjectCatalog {
    objects: Vec<ObjectDescriptor>,
    names: NameComparison,
    scope: CatalogScope,
}
impl ObjectCatalog {
    /// Validate caller-selected size bound, scoped references and unique IDs.
    /// Duplicate names are preserved; lookup reports ambiguity under its filters.
    pub fn new(
        scope: CatalogScope,
        objects: Vec<ObjectDescriptor>,
        names: NameComparison,
        max_objects: usize,
    ) -> Result<Self, ModelError> {
        if objects.len() > max_objects {
            return Err(ModelError::Limit);
        }
        let mut ids = HashSet::new();
        ids.try_reserve(objects.len())
            .map_err(|_| ModelError::Allocation)?;
        for object in &objects {
            if !scope.contains(object.id()) || object.parent().is_some_and(|id| !scope.contains(id))
            {
                return Err(ModelError::Scope);
            }
            if !ids.insert(object.id()) {
                return Err(ModelError::DuplicateIdentity);
            }
        }
        Ok(Self {
            objects,
            names,
            scope,
        })
    }
    /// Caller-issued catalog namespace.
    pub const fn scope(&self) -> CatalogScope {
        self.scope
    }
    /// Objects in adapter-provided order, including unknown categories.
    pub fn objects(&self) -> &[ObjectDescriptor] {
        &self.objects
    }
    /// Resolve an exact semantic identity; names and native IDs are not substitutes.
    pub fn get(&self, id: ObjectId) -> Option<&ObjectDescriptor> {
        self.objects.iter().find(|o| o.id() == id)
    }
    /// Lookup by explicit name policy and optional category/parent filters.
    /// None for a filter means any category/parent; no match returns None.
    pub fn lookup(
        &self,
        name: &str,
        kind: Option<ObjectKind>,
        parent: Option<ObjectId>,
    ) -> Result<Option<&ObjectDescriptor>, ModelError> {
        let mut matches = self.objects.iter().filter(|o| {
            self.names.equals(o.name().as_str(), name)
                && kind.is_none_or(|k| o.kind() == k)
                && parent.is_none_or(|p| o.parent() == Some(p))
        });
        let first = matches.next();
        if matches.next().is_some() {
            return Err(ModelError::AmbiguousName);
        }
        Ok(first)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_schema::SchemaName;
    fn object(scope: CatalogScope, id: u64, name: &str, kind: ObjectKind) -> ObjectDescriptor {
        ObjectDescriptor::new(
            scope.object_id(id),
            SchemaName::new(name.into()).unwrap(),
            kind,
            Some(scope.object_id(100)),
        )
    }
    #[test]
    fn identities_are_scoped_and_duplicate_names_require_disambiguation() {
        let scope = CatalogScope::new(7);
        let other = CatalogScope::new(8);
        assert_ne!(scope.object_id(u64::MAX), other.object_id(u64::MAX));
        let catalog = ObjectCatalog::new(
            scope,
            vec![
                object(scope, 1, "Orders", ObjectKind::Table),
                object(scope, 2, "orders", ObjectKind::Form),
                object(scope, 3, "Opaque", ObjectKind::Unknown),
            ],
            NameComparison::AsciiInsensitive,
            3,
        )
        .unwrap();
        assert!(catalog.get(other.object_id(1)).is_none());
        assert_eq!(
            catalog.lookup("ORDERS", None, None).err(),
            Some(ModelError::AmbiguousName)
        );
        assert_eq!(
            catalog
                .lookup("Orders", Some(ObjectKind::Form), Some(scope.object_id(100)))
                .unwrap()
                .unwrap()
                .id(),
            scope.object_id(2)
        );
        assert!(catalog.lookup("Missing", None, None).unwrap().is_none());
        assert!(catalog.get(scope.object_id(100)).is_none());
        assert_eq!(catalog.objects()[2].kind(), ObjectKind::Unknown);
        let exact = ObjectCatalog::new(
            scope,
            vec![object(scope, 1, "Orders", ObjectKind::Table)],
            NameComparison::Exact,
            3,
        )
        .unwrap();
        assert!(exact.lookup("orders", None, None).unwrap().is_none());
    }
    #[test]
    fn catalog_rejects_size_scope_and_duplicate_identity_errors() {
        let s = CatalogScope::new(1);
        let o = object(s, 1, "a", ObjectKind::Table);
        assert_eq!(
            ObjectCatalog::new(s, vec![o.clone()], NameComparison::Exact, 0).err(),
            Some(ModelError::Limit)
        );
        assert_eq!(
            ObjectCatalog::new(s, vec![o.clone(), o.clone()], NameComparison::Exact, 3).err(),
            Some(ModelError::DuplicateIdentity)
        );
        assert_eq!(
            ObjectCatalog::new(CatalogScope::new(2), vec![o], NameComparison::Exact, 3).err(),
            Some(ModelError::Scope)
        );
    }
}
