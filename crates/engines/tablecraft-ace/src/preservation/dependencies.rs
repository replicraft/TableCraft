use tablecraft_model::{ObjectDescriptor, ObjectKind};

/// Inert catalog evidence, never a complete dependency graph or write permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependencyCoverage {
    /// A module declaration is present; its execution is unavailable.
    ModuleExecutionUnavailable,
    /// Form/report/macro behavior has not been decoded or made executable.
    ApplicationBehaviorUnavailable,
    /// No conclusion about VBA, callbacks, data macros or external effects.
    Uninspected,
}

/// A borrowed object declaration and conservative behavior coverage.
pub struct ObjectDependency<'a> {
    pub(super) object: &'a ObjectDescriptor,
}
impl ObjectDependency<'_> {
    /// Original catalog declaration; bodies remain opaque in the retained image.
    pub fn object(&self) -> &ObjectDescriptor {
        self.object
    }

    /// What catalog metadata establishes about unavailable behavior.
    pub fn coverage(&self) -> DependencyCoverage {
        match self.object.kind() {
            ObjectKind::Module => DependencyCoverage::ModuleExecutionUnavailable,
            ObjectKind::Form | ObjectKind::Report | ObjectKind::Macro => {
                DependencyCoverage::ApplicationBehaviorUnavailable
            }
            _ => DependencyCoverage::Uninspected,
        }
    }
}
