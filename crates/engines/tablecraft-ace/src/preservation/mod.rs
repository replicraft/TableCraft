mod children;
mod dependencies;
mod image;
mod payload_data;
mod payloads;
pub(crate) mod table_extra;

pub use dependencies::{DependencyCoverage, ObjectDependency};
pub use image::NativeImage;
pub use payload_data::{ObjectPayloadData, OpaquePayload};
pub use payloads::{ObjectPayloads, PayloadPresence};
pub use table_extra::{
    DataMacroEnvelope, DataMacroProgram, DataMacroXml, DataMacroXmlNamespace, MacroProgramCoverage,
    MacroProgramDiagnostic, MacroProgramNode, MacroProgramNodeKind, MacroProgramProfile,
    MacroProgramText, MacroProgramTextKind, MacroPropertyInventory, MacroPropertyObservation,
    TableExtra, TableExtraCoverage, TableExtraGroup, TableExtraProperty, TableMacroInventory,
    XmlByteSpan,
};

pub(crate) const COPY_CHUNK_BYTES: usize = 64 * 1024;
