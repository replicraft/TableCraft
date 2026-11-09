//! TCQE0002 real-file observations use independent raw complete-image oracles.
mod framing;
mod recovery;
mod staging;

use super::{SCOPE, budget, open, retired_bundle};
use crate::editing::offline::{
    Cause, CompletionKind, OfflineWriteSession, Phase, QueryChange, SessionState, VerifiedImage,
    plan::MutationIntent,
    test_support::{assert_completion, assert_staged_read_state},
};
use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{
    NativeImage, QueryAttribute, QueryExpressionEditsIntentError, QueryFragmentExpressionEdit,
    RecoverableNativeQueryExpressionEditsPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::{
    IoError, SharedFile,
    journal::{RecoveryOutcome, filesystem::OfflineJournalPair},
};
use tablecraft_testkit::fixtures::access::query_expression_edits as raw;

fn review(before: &[u8], changed: bool) -> RecoverableNativeQueryExpressionEditsPlan {
    let edits = [
        QueryFragmentExpressionEdit {
            attribute: QueryAttribute::Column,
            order: [0, 0, 0, 2],
            expected: "99",
            replacement: if changed { "7000" } else { "99" },
        },
        QueryFragmentExpressionEdit {
            attribute: QueryAttribute::Column,
            order: [0, 0, 0, 1],
            expected: "42",
            replacement: if changed { "1000" } else { "42" },
        },
    ];
    NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
        .plan_recoverable_query_expression_edits("Saved", &edits, &mut budget(), &|| false)
        .unwrap()
}

fn after(before: &[u8]) -> Vec<u8> {
    raw::replace_at(&raw::replace_at(before, 4, "7000"), 3, "1000")
}

fn retained_pair(
    path: &Path,
    bundle: &Path,
    before: &[u8],
    committed: bool,
    retired: bool,
) -> OfflineWriteSession {
    fs::write(path, before).unwrap();
    let plan = review(before, true);
    let intent = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert_eq!(&intent[..8], b"TCQE0002");
    let mut owner = open(path);
    let slot = owner.slot_mut().unwrap();
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        bundle,
        intent,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    {
        let (mut target, mut journal) = pair.storage().unwrap();
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else if retired {
            assert_eq!(
                prepared.rollback(&mut budget(), &|| false).unwrap(),
                RecoveryOutcome::RolledBack
            );
        }
    }
    if retired {
        pair.retire(if committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        })
        .unwrap();
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    owner
}
