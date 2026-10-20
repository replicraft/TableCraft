use super::{Error, TableOutput, line};
use std::io::{Read, Seek};
use tablecraft_ace::{EncryptionInspection, inspect_encryption};

pub(super) fn run<R: Read + Seek>(source: &mut R, output: &mut TableOutput) -> Result<(), Error> {
    let inspection =
        inspect_encryption(source, &mut crate::native::budget(), &|| false).map_err(Error::Read)?;
    match inspection {
        EncryptionInspection::UnencodedPages => {
            line(output, &["Page encoding", "Unencoded"])?;
            line(output, &["Password protection", "Not determined"])?;
        }
        EncryptionInspection::JetEncoding => {
            line(output, &["Page encoding", "Jet encoding"])?;
            line(output, &["Password protection", "Not determined"])?;
            line(output, &["Page decoding", "Not implemented"])?;
        }
        EncryptionInspection::Office(info) => {
            line(output, &["Page encoding", "Office encryption"])?;
            let (major, minor) = info.version();
            line(output, &["Envelope version", &format!("{major}.{minor}")])?;
            line(
                output,
                &["Envelope family", &format!("{:?}", info.family())],
            )?;
            line(
                output,
                &["Descriptor bytes", &info.descriptor_bytes().to_string()],
            )?;
            if let Some(flags) = info.flags() {
                line(output, &["Outer flags", &format!("0x{flags:08x}")])?;
            }
            line(output, &["Cipher configuration", "Not validated"])?;
            line(output, &["Password verification", "Not implemented"])?;
            line(output, &["Page decoding", "Not implemented"])?;
        }
    }
    Ok(())
}
