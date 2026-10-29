mod decode;
mod inspect;
mod macro_xml;
mod model;

pub use macro_xml::{DataMacroEnvelope, DataMacroXml, DataMacroXmlNamespace};
pub use model::{TableExtra, TableExtraCoverage, TableExtraGroup, TableExtraProperty};

#[cfg(test)]
mod tests;
