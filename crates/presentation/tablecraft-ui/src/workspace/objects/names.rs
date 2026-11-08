//! Presentation-only names use the already loaded shared table/query namespace.
use super::MAX_RETAINED_BYTES;
use crate::workspace::Database;

const TABLE_PREFIX: &str = "Table";

pub(super) fn suggested_table_name(database: &Database) -> Option<String> {
    let candidates = database
        .tables
        .len()
        .checked_add(database.queries.len())?
        .checked_add(1)?;
    // One flag per loaded object bounds both allocation and the linear scan.
    // At least one candidate is unused by the pigeonhole principle.
    if candidates > MAX_RETAINED_BYTES {
        return None;
    }
    let mut occupied = Vec::new();
    occupied.try_reserve_exact(candidates).ok()?;
    occupied.resize(candidates, false);
    let names = database
        .tables
        .iter()
        .map(|table| table.name.as_str())
        .chain(database.queries.iter().map(|query| query.name.as_str()));
    for name in names {
        if !name
            .get(..TABLE_PREFIX.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(TABLE_PREFIX))
        {
            continue;
        }
        let suffix = &name[TABLE_PREFIX.len()..];
        if suffix.starts_with('0') || !suffix.as_bytes().iter().all(u8::is_ascii_digit) {
            continue;
        }
        if let Ok(number) = suffix.parse::<usize>()
            && let Some(flag) = number
                .checked_sub(1)
                .and_then(|index| occupied.get_mut(index))
        {
            *flag = true;
        }
    }
    let number = occupied.iter().position(|used| !used)?.checked_add(1)?;
    Some(format!("{TABLE_PREFIX}{number}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{SavedQuery, Table};

    #[test]
    fn lowest_unused_suggestion_respects_tables_queries_and_ascii_case_without_normalizing_names() {
        let mut database = Database {
            document_revision: 1,
            title: String::new(),
            title_path: None,
            format: String::new(),
            tables: Vec::new(),
            queries: Vec::new(),
        };
        assert_eq!(suggested_table_name(&database).as_deref(), Some("Table1"));
        for name in [
            "table1",
            "Table3",
            "Table0002",
            "Table+2",
            "Table0",
            "Table999999999999999999999999",
            "Items",
            "表Table2",
        ] {
            database.tables.push(Table {
                name: name.into(),
                local: true,
                system: false,
                storage: "Local".into(),
            });
        }
        assert_eq!(suggested_table_name(&database).as_deref(), Some("Table2"));
        database.queries.push(SavedQuery {
            name: "tAbLe2".into(),
            object_id: 9,
            system: false,
        });
        assert_eq!(suggested_table_name(&database).as_deref(), Some("Table4"));
    }
}
