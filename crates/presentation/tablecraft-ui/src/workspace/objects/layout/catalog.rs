//! Metadata comparison detects a wrong publication; it supplies no native proof.
use crate::Database;
use crate::workspace::objects::memory::Counter;

#[derive(Clone, Copy)]
pub(super) enum Change<'a> {
    Unchanged,
    TableDropped(&'a str),
    TableCreated(&'a str),
}

pub(super) fn matches(old: &Database, fresh: &Database) -> bool {
    matches_change(old, fresh, Change::Unchanged)
}

pub(super) fn matches_change(old: &Database, fresh: &Database, change: Change<'_>) -> bool {
    fresh.document_revision > old.document_revision
        && fresh.format == old.format
        && fresh.title_path == old.title_path
        && match change {
            Change::Unchanged => fresh.tables.len() == old.tables.len(),
            Change::TableCreated(name) => {
                old.tables.iter().all(|table| table.name != name)
                    && old.tables.len().checked_add(1) == Some(fresh.tables.len())
                    && fresh
                        .tables
                        .iter()
                        .filter(|table| table.name == name && table.local && !table.system)
                        .count()
                        == 1
                    && fresh
                        .tables
                        .iter()
                        .filter(|table| table.name == name)
                        .count()
                        == 1
            }
            Change::TableDropped(name) => {
                old.tables
                    .iter()
                    .filter(|table| table.name == name && table.local && !table.system)
                    .count()
                    == 1
                    && old.tables.iter().filter(|table| table.name == name).count() == 1
                    && fresh.tables.len().checked_add(1) == Some(old.tables.len())
                    && fresh.tables.iter().all(|table| table.name != name)
            }
        }
        && fresh.queries.len() == old.queries.len()
        && old
            .tables
            .iter()
            .filter(|table| match change {
                Change::Unchanged | Change::TableCreated(_) => true,
                Change::TableDropped(name) => table.name != name,
            })
            .zip(fresh.tables.iter().filter(|table| match change {
                Change::TableCreated(name) => table.name != name,
                Change::Unchanged | Change::TableDropped(_) => true,
            }))
            .all(|(old, fresh)| {
                old.name == fresh.name
                    && old.local == fresh.local
                    && old.system == fresh.system
                    && old.storage == fresh.storage
            })
        && old.queries.iter().zip(&fresh.queries).all(|(old, fresh)| {
            old.object_id == fresh.object_id && old.name == fresh.name && old.system == fresh.system
        })
}

pub(super) fn retained_bytes(database: &Database) -> Option<usize> {
    let mut count = Counter::new(size_of::<Database>());
    count.optional(&database.title_path);
    count.string(&database.title);
    count.string(&database.format);
    count.vector(&database.tables);
    for table in &database.tables {
        count.string(&table.name);
        count.string(&table.storage);
    }
    count.vector(&database.queries);
    for query in &database.queries {
        count.string(&query.name);
    }
    count.finish()
}
