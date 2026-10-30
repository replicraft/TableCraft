mod decode;
mod inspect;
mod macro_xml;
mod model;

pub use macro_xml::{
    DataMacroEnvelope, DataMacroProgram, DataMacroXml, DataMacroXmlNamespace, MacroProgramCoverage,
    MacroProgramDiagnostic, MacroProgramNode, MacroProgramNodeKind, MacroProgramProfile,
    XmlByteSpan,
};
pub use model::{TableExtra, TableExtraCoverage, TableExtraGroup, TableExtraProperty};

#[cfg(test)]
mod program_fixtures;
#[cfg(test)]
mod tests;
