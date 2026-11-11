#![cfg(feature = "journal")]

#[path = "journal_files/resizable.rs"]
mod resizable;

use std::{
    fs::{self, OpenOptions},
    io::Write,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::{
    ExclusiveFile,
    journal::{RecoveryOutcome, prepare, recover},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

#[test]
fn retained_journals_reopen_and_recover_over_locked_real_files() {
    let directory = tempfile::tempdir().unwrap();

    for committed in [false, true] {
        let target_path = directory.path().join(format!("target-{committed}"));
        let journal_path = directory.path().join(format!("journal-{committed}"));
        let mut target = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target_path)
            .unwrap();
        target.write_all(b"original").unwrap();
        target.sync_all().unwrap();
        let journal = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&journal_path)
            .unwrap();
        journal.sync_all().unwrap();
        drop(target);
        drop(journal);

        // This exercises Unix directory synchronization, not power-loss claims.
        #[cfg(unix)]
        fs::File::open(directory.path())
            .unwrap()
            .sync_all()
            .unwrap();

        let mut target = ExclusiveFile::open(&target_path).unwrap();
        let mut journal = ExclusiveFile::open(&journal_path).unwrap();
        let prepared = prepare(
            &mut target,
            &mut journal,
            b"modified",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
        }
        target.close().unwrap();
        journal.close().unwrap();

        let mut target = ExclusiveFile::open(&target_path).unwrap();
        let mut journal = ExclusiveFile::open(&journal_path).unwrap();
        let expected = if committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        };
        assert_eq!(
            recover(&mut target, &mut journal, &mut budget(), &|| false).unwrap(),
            expected
        );
        target.close().unwrap();
        journal.close().unwrap();

        assert_eq!(
            fs::read(&target_path).unwrap(),
            if committed { b"modified" } else { b"original" }
        );
        assert!(fs::metadata(&journal_path).unwrap().len() > 0);
    }
}
