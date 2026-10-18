use super::{NativeImage, ObjectDependency};
use crate::{InspectError, header::check};
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};

impl NativeImage {
    /// Inspect direct catalog containment for an exact captured object identity.
    /// Unknown roots fail rather than appearing dependency-free. This is not a
    /// complete dependency graph: streams, callbacks and indirect references stay
    /// uninspected. No child body, module, provider or application code executes.
    pub fn inspect_object_children(
        &self,
        parent: ObjectId,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<ObjectDependency<'_>>, InspectError> {
        let _span = tracing::debug_span!("native.object.children").entered();
        check(budget, cancelled)?;
        let mut known = false;
        let mut count = 0;
        for object in self.catalog().catalog().objects() {
            check(budget, cancelled)?;
            known |= object.id() == parent;
            count += usize::from(object.parent() == Some(parent));
        }
        if !known {
            return Err(InspectError::UnknownObject);
        }
        budget.charge_allocation(count as u64, size_of::<ObjectDependency<'_>>() as u64)?;
        let mut children = Vec::new();
        children
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        for object in self.catalog().catalog().objects() {
            check(budget, cancelled)?;
            if object.parent() != Some(parent) {
                continue;
            }
            children.push(ObjectDependency { object });
        }
        check(budget, cancelled)?;
        Ok(children)
    }
}
