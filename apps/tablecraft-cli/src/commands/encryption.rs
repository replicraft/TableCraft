use super::error::Error;
use crate::output::TableOutput;
use std::io::{Read, Seek};
use tablecraft_ace::{EncryptionInspection, inspect_encryption};
mod copying;
mod resources;

#[derive(clap::Subcommand)]
#[command(
    after_help = "Add, remove and change-password require a stable/offline source and a fresh explicit output path. They preserve the source and stream page batches using automatic parallelism and available RAM with reserved headroom. --jobs and --memory-budget-mib override worker count and cap working buffers; cumulative work remains bounded by source size and native limits. Jet 4 password fields are separate from its page encoding. Unsupported schemes or SID layouts fail without publishing an output."
)]
pub(crate) enum Command {
    /// Report inert encoding/encryption envelope metadata; does not verify passwords.
    Inspect {
        /// Explicit MDB/ACCDB source path.
        path: std::path::PathBuf,
    },
    /// Add encryption into a fresh output (Agile AES-256 for ACCDB; legacy encoding for Jet 4).
    Add {
        path: std::path::PathBuf,
        #[arg(long, value_name = "FILE")]
        output: std::path::PathBuf,
        /// New password; prompts with hidden confirmation when omitted.
        #[arg(long, value_name = "PASSWORD")]
        new_password: Option<String>,
        #[command(flatten)]
        resources: resources::Options,
    },
    /// Remove encryption and the database password into a fresh output file.
    Remove {
        path: std::path::PathBuf,
        #[arg(long, value_name = "FILE")]
        output: std::path::PathBuf,
        #[command(flatten)]
        resources: resources::Options,
    },
    /// Change the database password into a fresh output, retaining its cipher scheme.
    ChangePassword {
        path: std::path::PathBuf,
        #[arg(long, value_name = "FILE")]
        output: std::path::PathBuf,
        /// New password; prompts with hidden confirmation when omitted.
        #[arg(long, value_name = "PASSWORD")]
        new_password: Option<String>,
        #[command(flatten)]
        resources: resources::Options,
    },
}

impl Command {
    pub(crate) fn run(
        self,
        format: crate::presentation::Format,
        password: Option<&str>,
    ) -> Result<String, Error> {
        match self {
            Self::Remove {
                path,
                output,
                resources,
            } => copying::run(
                &path,
                &output,
                password,
                copying::Operation::Remove,
                None,
                resources,
            ),
            Self::ChangePassword {
                path,
                output,
                new_password,
                resources,
            } => copying::run(
                &path,
                &output,
                password,
                copying::Operation::ChangePassword,
                new_password.map(zeroize::Zeroizing::new),
                resources,
            ),
            Self::Add {
                path,
                output,
                new_password,
                resources,
            } => copying::run(
                &path,
                &output,
                password,
                copying::Operation::Add,
                new_password.map(zeroize::Zeroizing::new),
                resources,
            ),
            Self::Inspect { path } => {
                let mut source = crate::native::open_raw(&path, &mut crate::native::budget())
                    .map_err(Error::Input)?;
                let mut output = TableOutput::new();
                if source.is_deployment() {
                    line(&mut output, &["Container", "Access deployment (CAB)"])?;
                    line(&mut output, &["Package signature", "Not verified"])?;
                }
                report(&mut source, &mut output)?;
                let (text, sections) = output.finish_sections();
                format.render_report(text, &sections).map_err(Error::Output)
            }
        }
    }
}

pub(super) fn summary<R: Read + Seek>(
    source: &mut R,
    output: &mut TableOutput,
) -> Result<(), Error> {
    let inspection = inspect_encryption(source, &mut crate::native::budget(), &|| false);
    let (encrypted, kind) = match inspection {
        Ok(EncryptionInspection::UnencodedPages) => ("No", "None".to_owned()),
        Ok(EncryptionInspection::JetEncoding) => ("Encoded", "Jet encoding".to_owned()),
        Ok(EncryptionInspection::Office(info)) => ("Yes", format!("Office {:?}", info.family())),
        Err(tablecraft_ace::InspectError::Unsupported) => {
            ("Not determined", "Unqualified".to_owned())
        }
        Err(error) => return Err(Error::Read(error)),
    };
    line(output, &["Page encryption", encrypted])?;
    line(output, &["Encryption type", &kind])
}

fn line(output: &mut TableOutput, columns: &[&str]) -> Result<(), Error> {
    for (i, text) in columns.iter().enumerate() {
        if i > 0 {
            output.separator().map_err(Error::Output)?;
        }
        output.header(Some(text), i + 1).map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)
}

fn report<R: Read + Seek>(source: &mut R, output: &mut TableOutput) -> Result<(), Error> {
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
            line(output, &["Page decoding", "Not performed"])?;
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
            line(output, &["Password verification", "Not performed"])?;
            line(output, &["Page decoding", "Not performed"])?;
        }
    }
    Ok(())
}
