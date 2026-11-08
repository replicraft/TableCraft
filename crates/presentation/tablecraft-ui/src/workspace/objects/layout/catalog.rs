//! Metadata comparison detects a wrong publication; it supplies no native proof.
use crate::Database;
use crate::workspace::objects::memory::Counter;

pub(super) fn matches(old: &Database, fresh: &Database) -> bool {
    fresh.document_revision > old.document_revision
        && fresh.format == old.format
        && fresh.title_path == old.title_path
        && fresh.tables.len() == old.tables.len()
        && fresh.queries.len() == old.queries.len()
        && old.tables.iter().zip(&fresh.tables).all(|(old, fresh)| {
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
