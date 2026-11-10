//! Host-owned scheduling; release trust and installation policy live in the updater.

mod worker;

pub(crate) use worker::{Reply, RetainedOutcome, UpdateWorker};
