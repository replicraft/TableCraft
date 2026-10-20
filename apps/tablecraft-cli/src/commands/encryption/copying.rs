use super::Error;
use std::{fs::File, io::IsTerminal, path::Path};
use tablecraft_ace::{
    EncryptionRewrite, InspectError, check_encryption_rewrite, rewrite_encryption,
};
use tablecraft_core::limits::ResourceBudget;
use zeroize::Zeroizing;

const REWRITE_BYTES: u64 = 512 * 1024 * 1024;
const REWRITE_ROWS: u64 = 100_000;
const REWRITE_STEPS: u64 = 100_000_000;
const COPY_CHUNK_BYTES: usize = 64 * 1024;

pub(super) fn run(
    input: &Path,
    output: &Path,
    password: Option<&str>,
    change: bool,
    new_password: Option<Zeroizing<String>>,
) -> Result<String, Error> {
    let _span = tracing::debug_span!("cli.encryption.copy", change_password = change).entered();
    if output.try_exists().map_err(|e| Error::Copy(e.kind()))? {
        return Err(Error::Copy(std::io::ErrorKind::AlreadyExists));
    }
    let mut source = File::open(input).map_err(|e| Error::Read(e.into()))?;
    let mut budget = ResourceBudget::new(REWRITE_BYTES, REWRITE_ROWS, REWRITE_STEPS);
    let mut entered = None;
    match check_encryption_rewrite(&mut source, password, &mut budget, &|| false) {
        Err(InspectError::PasswordRequired)
            if password.is_none() && std::io::stdin().is_terminal() =>
        {
            entered = Some(prompt("Database password: ")?);
            check_encryption_rewrite(
                &mut source,
                entered.as_ref().map(|p| p.as_str()),
                &mut budget,
                &|| false,
            )
            .map_err(Error::Read)?;
        }
        result => result.map_err(Error::Read)?,
    }
    let password = password.or_else(|| entered.as_ref().map(|p| p.as_str()));
    let new_password = if change {
        if let Some(password) = new_password {
            Some(password)
        } else {
            if !std::io::stdin().is_terminal() {
                return Err(Error::NewPasswordRequired);
            }
            let first = prompt("New database password: ")?;
            let confirmation = prompt("Confirm new database password: ")?;
            if *first != *confirmation {
                return Err(Error::PasswordMismatch);
            }
            Some(first)
        }
    } else {
        None
    };
    let operation = if let Some(new) = &new_password {
        EncryptionRewrite::ChangePassword {
            password,
            new_password: new.as_str(),
        }
    } else {
        EncryptionRewrite::Remove { password }
    };
    let copy =
        rewrite_encryption(&mut source, operation, &mut budget, &|| false).map_err(Error::Read)?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|e| Error::Copy(e.kind()))?;
    for (number, chunk) in copy.as_bytes().chunks(COPY_CHUNK_BYTES).enumerate() {
        tablecraft_io::write_exact_at(
            temporary.as_file_mut(),
            (number * COPY_CHUNK_BYTES) as u64,
            chunk,
            &mut budget,
            &|| false,
        )
        .map_err(|e| Error::Read(e.into()))?;
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| Error::Copy(e.kind()))?;
    temporary
        .persist_noclobber(output)
        .map_err(|e| Error::Copy(e.error.kind()))?;
    Ok(if change {
        "Database password changed\n"
    } else {
        "Database encryption removed\n"
    }
    .to_owned())
}

fn prompt(label: &str) -> Result<Zeroizing<String>, Error> {
    rpassword::prompt_password(label)
        .map(Zeroizing::new)
        .map_err(|_| Error::Read(InspectError::PasswordRequired))
}
