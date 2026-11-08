use super::Error;
#[cfg(not(target_family = "wasm"))]
use rayon::ThreadPoolBuilder;
use std::{io::IsTerminal, path::Path};
#[cfg(not(target_family = "wasm"))]
use tablecraft_ace::rewrite_encryption_parallel;
use tablecraft_ace::{
    EncryptionRewrite, InspectError, check_encryption_add, check_encryption_rewrite,
    rewrite_encryption_to,
};
#[cfg(not(target_family = "wasm"))]
use tablecraft_core::limits::Resource;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::SharedFile;
use zeroize::Zeroizing;

const REWRITE_BYTES: u64 = 512 * 1024 * 1024;
const REWRITE_ROWS: u64 = 100_000;
const REWRITE_STEPS: u64 = 100_000_000;

#[derive(Clone, Copy)]
pub(super) enum Operation {
    Remove,
    ChangePassword,
    Add,
}

pub(super) fn run(
    input: &Path,
    output: &Path,
    password: Option<&str>,
    operation: Operation,
    new_password: Option<Zeroizing<String>>,
    jobs: u8,
) -> Result<String, Error> {
    let _span = tracing::debug_span!("cli.encryption.copy").entered();
    if output.try_exists().map_err(|e| Error::Copy(e.kind()))? {
        return Err(Error::Copy(std::io::ErrorKind::AlreadyExists));
    }
    let mut source = SharedFile::open(input).map_err(|e| Error::Read(e.into()))?;
    let mut budget = ResourceBudget::new(REWRITE_BYTES, REWRITE_ROWS, REWRITE_STEPS);
    let preflight = match operation {
        Operation::Add => check_encryption_add::<SharedFile>,
        Operation::Remove | Operation::ChangePassword => check_encryption_rewrite::<SharedFile>,
    };
    let mut entered = None;
    match preflight(&mut source, password, &mut budget, &|| false) {
        Err(InspectError::PasswordRequired)
            if password.is_none() && std::io::stdin().is_terminal() =>
        {
            entered = Some(prompt("Database password: ")?);
            preflight(
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
    let new_password = if matches!(operation, Operation::ChangePassword | Operation::Add) {
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
    let rewrite = match operation {
        Operation::Add => EncryptionRewrite::Add {
            password,
            new_password: new_password
                .as_ref()
                .ok_or(Error::NewPasswordRequired)?
                .as_str(),
        },
        Operation::ChangePassword => EncryptionRewrite::ChangePassword {
            password,
            new_password: new_password
                .as_ref()
                .ok_or(Error::NewPasswordRequired)?
                .as_str(),
        },
        Operation::Remove => EncryptionRewrite::Remove { password },
    };
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|e| Error::Copy(e.kind()))?;
    if jobs > 1 {
        parallel_remove(
            &mut source,
            temporary.as_file_mut(),
            rewrite,
            jobs,
            &mut budget,
        )?;
    } else {
        rewrite_encryption_to(
            &mut source,
            temporary.as_file_mut(),
            rewrite,
            &mut budget,
            &|| false,
        )
        .map_err(Error::Read)?;
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| Error::Copy(e.kind()))?;
    temporary
        .persist_noclobber(output)
        .map_err(|e| Error::Copy(e.error.kind()))?;
    Ok(match operation {
        Operation::Add => "Database encryption added\n",
        Operation::ChangePassword => "Database password changed\n",
        Operation::Remove => "Database encryption removed\n",
    }
    .to_owned())
}

#[cfg(not(target_family = "wasm"))]
fn parallel_remove(
    source: &mut SharedFile,
    target: &mut std::fs::File,
    rewrite: EncryptionRewrite<'_>,
    jobs: u8,
    budget: &mut ResourceBudget,
) -> Result<(), Error> {
    let pool = ThreadPoolBuilder::new()
        .num_threads(usize::from(jobs))
        .build()
        .map_err(|_| Error::Read(InspectError::Allocation))?;
    let copy = rewrite_encryption_parallel(source, rewrite, &pool, budget, &|| false)
        .map_err(Error::Read)?;
    budget
        .charge(Resource::Bytes, copy.as_bytes().len() as u64)
        .map_err(|error| Error::Read(error.into()))?;
    tablecraft_io::write_exact_at(target, 0, copy.as_bytes(), budget, &|| false)
        .map_err(|error| Error::Read(error.into()))
}

#[cfg(target_family = "wasm")]
fn parallel_remove(
    _: &mut SharedFile,
    _: &mut std::fs::File,
    _: EncryptionRewrite<'_>,
    _: u8,
    _: &mut ResourceBudget,
) -> Result<(), Error> {
    Err(Error::Read(InspectError::UnsupportedEncryption))
}

fn prompt(label: &str) -> Result<Zeroizing<String>, Error> {
    rpassword::prompt_password(label)
        .map(Zeroizing::new)
        .map_err(|_| Error::Read(InspectError::PasswordRequired))
}
