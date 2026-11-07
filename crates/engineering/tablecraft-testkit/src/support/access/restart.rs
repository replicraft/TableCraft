//! Create real paired image journals for independently hosted restart scenarios.
use crate::fixtures::access::mutation_fixture;
#[cfg(feature = "ace16-create-support")]
use crate::fixtures::access::{canonical_blank_creation, canonical_initial_table_creation};
use std::{fs, io::Cursor, path::Path};
#[cfg(feature = "ace16-create-support")]
use tablecraft_ace::{CreateColumn, FirstTableDefinition};
use tablecraft_ace::{Int32Update, NativeImage};
use tablecraft_core::limits::ResourceBudget;
#[cfg(feature = "ace16-create-support")]
use tablecraft_io::journal::ResizableDurableStorage;
use tablecraft_io::{
    journal::{
        DurableStorage,
        filesystem::{OfflineJournalPair, OfflineTarget},
    },
    write_exact_at,
};
use tablecraft_model::CatalogScope;
#[cfg(feature = "ace16-create-support")]
use tablecraft_schema::ScalarType;

pub struct RestartImages {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(100_000_000, 1_000_000, 100_000_000)
}

/// Leaves the actual active paired journal and releases every owner for restart.
/// Pending represents a crash after the complete target write/sync but before a
/// commit marker. No production encoder supplies the complete AFTER byte oracle.
pub fn stage(path: &Path, journal: &Path, committed: bool) -> RestartImages {
    let before = mutation_fixture::database();
    let mut after = before.clone();
    let page = 12 * 4096;
    let slot = usize::from(u16::from_le_bytes(
        before[page + 14..page + 16].try_into().unwrap(),
    ));
    let score = page + slot + 6;
    assert_eq!(&before[score..score + 4], &10_i32.to_le_bytes());
    after[score..score + 4].copy_from_slice(&17_i32.to_le_bytes());
    let scope = CatalogScope::new(63);
    let plan = NativeImage::capture(&mut Cursor::new(&before), scope, &mut budget(), &|| false)
        .unwrap()
        .plan_recoverable_int32_updates(
            "Items",
            "Score",
            vec![Int32Update::new(0, 10, 17)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.after_bytes(), after);
    let intent = plan.intent().encode(&mut budget(), &|| false).unwrap();
    fs::write(path, &before).unwrap();
    let target = OfflineTarget::open(path, &mut budget(), &|| false).unwrap();
    let mut pair =
        OfflineJournalPair::create(target, journal, intent, &mut budget(), &|| false).unwrap();
    {
        let (mut target, mut journal) = pair.storage().unwrap();
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
            write_exact_at(&mut target, 0, &after, &mut budget(), &|| false).unwrap();
            target.sync_storage().unwrap();
        }
    }
    pair.release().unwrap();
    assert_eq!(fs::read(path).unwrap(), after);
    RestartImages { before, after }
}

/// Leave an actual V2 first-table journal with original owned declarations.
/// Canonical BEFORE/AFTER are raw independent fixtures, not creator output.
#[cfg(feature = "ace16-create-support")]
pub fn stage_first_table(path: &Path, journal: &Path, committed: bool) -> RestartImages {
    let before = canonical_blank_creation::source();
    let after =
        canonical_initial_table_creation::source(canonical_initial_table_creation::Case::ShortText)
            .bytes;
    let fields = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::short_text("Name", 255).unwrap(),
    ];
    let definition =
        FirstTableDefinition::capture("Items", &fields, &mut budget(), &|| false).unwrap();
    let scope = CatalogScope::new(63);
    let plan = NativeImage::capture(&mut Cursor::new(&before), scope, &mut budget(), &|| false)
        .unwrap()
        .plan_first_table(definition, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.before_bytes(), before);
    assert_eq!(plan.after_bytes(), after);
    let intent = plan.encode_intent(&mut budget(), &|| false).unwrap();
    fs::write(path, &before).unwrap();
    let target = OfflineTarget::open(path, &mut budget(), &|| false).unwrap();
    let mut pair =
        OfflineJournalPair::create(target, journal, intent, &mut budget(), &|| false).unwrap();
    {
        let (mut target, mut journal) = pair.resizable_storage().unwrap();
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
            write_exact_at(&mut target, 0, &after, &mut budget(), &|| false).unwrap();
            target.resize_storage(after.len() as u64).unwrap();
            target.sync_storage().unwrap();
        }
    }
    pair.release().unwrap();
    assert_eq!(fs::read(path).unwrap(), after);
    RestartImages { before, after }
}
