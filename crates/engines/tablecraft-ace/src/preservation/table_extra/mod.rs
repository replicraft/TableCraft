mod coverage;
mod decode;
mod inspect;
mod inventory;
mod macro_xml;
mod model;

pub use coverage::{MacroCoverageChild, TableMacroCoverage};
pub use inventory::{MacroPropertyInventory, MacroPropertyObservation, TableMacroInventory};
pub use macro_xml::{
    DataMacroEnvelope, DataMacroProgram, DataMacroXml, DataMacroXmlNamespace, MacroProgramCoverage,
    MacroProgramDiagnostic, MacroProgramNode, MacroProgramNodeKind, MacroProgramProfile,
    MacroProgramText, MacroProgramTextKind, XmlByteSpan,
};
pub use model::{TableExtra, TableExtraCoverage, TableExtraGroup, TableExtraProperty};

#[cfg(test)]
mod program_fixtures;
#[cfg(test)]
pub(crate) mod tests;
