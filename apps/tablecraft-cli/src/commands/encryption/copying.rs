#[cfg(not(target_family = "wasm"))]
use super::resources::Memory;
use super::{Error, resources::Options};
#[cfg(not(target_family = "wasm"))]
use rayon::ThreadPoolBuilder;
use std::{io::IsTerminal, path::Path};
#[cfg(target_family = "wasm")]
use tablecraft_ace::rewrite_encryption_to;
use tablecraft_ace::{
    EncryptionRewrite, InspectError, check_encryption_add, check_encryption_rewrite,
    encryption_rewrite_budget,
};
#[cfg(not(target_family = "wasm"))]
use tablecraft_ace::{ParallelEncryption, rewrite_encryption_parallel_to};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::SharedFile;
use zeroize::Zeroizing;

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
    resources: Options,
) -> Result<String, Error> {
    let _span = tracing::debug_span!("cli.encryption.copy").entered();
    if output.try_exists().map_err(|e| Error::Copy(e.kind()))? {
        return Err(Error::Copy(std::io::ErrorKind::AlreadyExists));
    }
    let mut source = SharedFile::open(input).map_err(|e| Error::Read(e.into()))?;
    let length = source.length().map_err(|error| Error::Read(error.into()))?;
    let mut budget = encryption_rewrite_budget(length).map_err(Error::Read)?;
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
    perform_rewrite(
        &mut source,
        temporary.as_file_mut(),
        rewrite,
        &resources,
        &mut budget,
    )?;
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
fn perform_rewrite(
    source: &mut SharedFile,
    target: &mut std::fs::File,
    operation: EncryptionRewrite<'_>,
    resources: &Options,
    budget: &mut ResourceBudget,
) -> Result<(), Error> {
    let mut memory = Memory::new(resources)?;
    let pool = ThreadPoolBuilder::new()
        .num_threads(resources.workers())
        .build()
        .map_err(|_| Error::Read(InspectError::Allocation))?;
    let mut allowance = |retained| memory.allowance(retained);
    let parallel = ParallelEncryption::new(&pool, &mut allowance);
    rewrite_encryption_parallel_to(source, target, operation, parallel, budget, &|| false)
        .map_err(Error::Read)
}

#[cfg(target_family = "wasm")]
fn perform_rewrite(
    source: &mut SharedFile,
    target: &mut std::fs::File,
    operation: EncryptionRewrite<'_>,
    _: &Options,
    budget: &mut ResourceBudget,
) -> Result<(), Error> {
    rewrite_encryption_to(source, target, operation, budget, &|| false).map_err(Error::Read)
}

fn prompt(label: &str) -> Result<Zeroizing<String>, Error> {
    rpassword::prompt_password(label)
        .map(Zeroizing::new)
        .map_err(|_| Error::Read(InspectError::PasswordRequired))
}
