//! Source-neutral reading of acknowledged facts, never an operation authority.
use super::{SaveKnownOutcome, SaveOutcome, SaveReceipt};

pub(super) struct ReopenFacts {
    pub(super) replacement: bool,
    pub(super) completed: Option<SaveReceipt>,
}

pub(super) fn known_replacement(known: SaveKnownOutcome) -> bool {
    matches!(
        known,
        SaveKnownOutcome::Committed | SaveKnownOutcome::ReplacementVerified
    )
}

/// The operation owner supplies its exact receipt predicate. A failed retained
/// owner cannot reopen; current Original wins over historical completion. A
/// released AFTER observation does not manufacture a missing completed receipt.
/// The caller still needs its distinct native full-image witness after ACK.
pub(super) fn reopen_facts(
    outcome: SaveOutcome,
    qualified_receipt: impl Fn(SaveReceipt) -> bool,
) -> Result<ReopenFacts, ()> {
    let (replacement, completed) = match outcome {
        SaveOutcome::Saved { receipt } => {
            if known_replacement(receipt.known) && !qualified_receipt(receipt) {
                return Err(());
            }
            let replacement = qualified_receipt(receipt);
            (replacement, replacement.then_some(receipt))
        }
        SaveOutcome::Released {
            current_outcome,
            last_completed,
        } => {
            let completed = last_completed.filter(|receipt| qualified_receipt(*receipt));
            let replacement = current_outcome.map_or(completed.is_some(), known_replacement);
            (replacement, completed)
        }
        SaveOutcome::Failed(_) => (false, None),
        SaveOutcome::Uncertain { .. } => return Err(()),
    };
    Ok(ReopenFacts {
        replacement,
        completed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncertain_actor_has_no_current_replacement_or_receipt_authority() {
        for previous in [
            None,
            Some(SaveKnownOutcome::OriginalVerified),
            Some(SaveKnownOutcome::Committed),
        ] {
            let outcome = SaveOutcome::Uncertain {
                last_known_outcome: previous,
                last_completed: None,
            };
            assert!(reopen_facts(outcome, |_| true).is_err());
        }
    }
}
