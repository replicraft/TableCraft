/// Observed XML namespace, separate from the file selector or Access version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataMacroXmlNamespace {
    /// Access Services application namespace dated April 2009.
    April2009,
    /// Access Services application namespace dated November 2009.
    November2009,
}

impl DataMacroXmlNamespace {
    /// Exact namespace URI represented by this observed family.
    pub const fn uri(self) -> &'static str {
        match self {
            Self::April2009 => {
                "http://schemas.microsoft.com/office/accessservices/2009/04/application"
            }
            Self::November2009 => {
                "http://schemas.microsoft.com/office/accessservices/2009/11/application"
            }
        }
    }
}

/// One explicit DataMacro element. Name and Event are original XML facts, not a
/// validated executable program or an event inferred from its property name.
pub struct DataMacroEnvelope {
    pub(super) event: Option<String>,
    pub(super) name: Option<String>,
}

impl DataMacroEnvelope {
    /// Normalized XML Event attribute, without inferring an executable event.
    pub fn event(&self) -> Option<&str> {
        self.event.as_deref()
    }

    /// Normalized XML Name attribute for an explicitly named macro.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

/// Complete bounded XML envelope inspection. Body grammar, expression/action
/// semantics, absence and event execution remain unqualified. No automatic
/// Debug/Serde output exposes stored program text or names.
pub struct DataMacroXml {
    pub(super) namespace: DataMacroXmlNamespace,
    pub(super) definitions: Vec<DataMacroEnvelope>,
}

impl DataMacroXml {
    /// Exact recognized namespace family, not the database format or engine build.
    pub const fn namespace(&self) -> DataMacroXmlNamespace {
        self.namespace
    }

    /// Explicit DataMacro envelopes in document order; bodies remain uninterpreted.
    pub fn definitions(&self) -> &[DataMacroEnvelope] {
        &self.definitions
    }
}
