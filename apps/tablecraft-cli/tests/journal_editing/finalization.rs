use super::{budget, images, leave_actual_pair_for_recovery};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tablecraft_ace::{NativeInt32EditIntent, qualify_jet4_int32_recovery};
use tablecraft_io::journal::{
    filesystem::{OfflineJournalPair, OfflineTarget},
    inspect_v1,
};
use tablecraft_model::CatalogScope;

fn finalize(database: &Path, journal: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "finalize-update"])
        .arg("--database")
        .arg(database)
        .arg("--journal")
        .arg(journal)
        .output()
        .unwrap()
}

fn retired_pair(root: &Path, committed: bool) -> (PathBuf, PathBuf) {
    let database = root.join("source.mdb");
    let active = root.join("active-journal");
    // Reuse the actual parent's public native-plan/paired-device setup. Its
    // original and complete expected image remain independently authored there.
    leave_actual_pair_for_recovery(&database, &active, committed);

    let mut work = budget();
    let target = OfflineTarget::open(&database, &mut work, &|| false).unwrap();
    let mut pair = OfflineJournalPair::open_existing(
        target,
        &active,
        NativeInt32EditIntent::MAX_ENCODED_BYTES as u64,
        &mut work,
        &|| false,
    )
    .unwrap();
    let intent = NativeInt32EditIntent::decode(pair.intent_bytes(), &mut work, &|| false).unwrap();
    let outcome = {
        let (mut target, mut journal) = pair.storage().unwrap();
        let checked = inspect_v1(&mut journal, &mut work, &|| false).unwrap();
        let qualified =
            qualify_jet4_int32_recovery(checked, intent, CatalogScope::new(1), &mut work, &|| {
                false
            })
            .unwrap();
        qualified
            .recover(&mut target, &mut work, &|| false)
            .unwrap()
    };
    pair.retire(outcome).unwrap();
    pair.release().unwrap();
    drop(pair);
    assert!(!active.exists());

    let directories: Vec<PathBuf> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    assert_eq!(directories.len(), 1);
    (database, directories.into_iter().next().unwrap())
}

fn remaining_records(directory: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut records: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();
    records.sort_by(|left, right| left.0.cmp(&right.0));
    records
}

#[test]
fn finalize_confirms_original_or_replacement_without_replaying_the_edit() {
    for committed in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (database, retired) = retired_pair(root.path(), committed);
        let (before, after) = images();
        let expected = if committed { after } else { before };
        assert_eq!(fs::read(&database).unwrap(), expected);

        let output = finalize(&database, &retired);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let expected_report: &[u8] = if committed {
            b"Verified image\tReplacement\nJournal namespace\tRetired and synchronized\n"
        } else {
            b"Verified image\tOriginal\nJournal namespace\tRetired and synchronized\n"
        };
        assert_eq!(output.stdout, expected_report);
        assert_eq!(fs::read(&database).unwrap(), expected);
        assert!(!retired.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn active_or_partly_cleaned_bundles_refuse_without_deleting_remaining_data() {
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("source.mdb");
    let active = root.path().join("active-journal");
    leave_actual_pair_for_recovery(&database, &active, false);
    let target = fs::read(&database).unwrap();
    let records = remaining_records(&active);
    let output = finalize(&database, &active);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read(&database).unwrap(), target);
    assert_eq!(remaining_records(&active), records);

    for missing in ["binding", "intent", "journal"] {
        let root = tempfile::tempdir().unwrap();
        let (database, retired) = retired_pair(root.path(), true);
        fs::remove_file(retired.join(missing)).unwrap();
        let target = fs::read(&database).unwrap();
        let records = remaining_records(&retired);
        let output = finalize(&database, &retired);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(fs::read(&database).unwrap(), target);
        assert_eq!(remaining_records(&retired), records);
    }
}

#[test]
fn opposite_current_images_are_not_repaired_by_a_hidden_recovery_fallback() {
    for committed in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (database, retired) = retired_pair(root.path(), committed);
        let (before, after) = images();
        // Both full images were independently qualified by the unchanged parent
        // SOURCE gate. The current image deliberately disagrees with V1 state.
        let opposite = if committed { before } else { after };
        fs::write(&database, &opposite).unwrap();
        let records = remaining_records(&retired);

        let output = finalize(&database, &retired);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(fs::read(&database).unwrap(), opposite);
        assert_eq!(remaining_records(&retired), records);
    }
}
