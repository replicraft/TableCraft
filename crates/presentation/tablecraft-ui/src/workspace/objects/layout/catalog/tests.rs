//! Inert catalog delta checks do not construct a native review or reopen witness.
use super::*;
use crate::{SavedQuery, Table};

const STORAGE: &str = "validated local storage presentation";

fn table(name: &str, system: bool) -> Table {
    Table {
        name: name.into(),
        local: true,
        system,
        storage: STORAGE.into(),
    }
}

fn before() -> Database {
    Database {
        title_path: Some("/catalog.accdb".into()),
        document_revision: 7,
        title: "Catalog".into(),
        format: "ACE16".into(),
        tables: vec![table("First", false), table("Second", false)],
        queries: vec![SavedQuery {
            object_id: 1,
            name: "Saved".into(),
            system: false,
        }],
    }
}

fn after() -> Database {
    let mut database = before();
    database.document_revision += 1;
    database.tables.insert(1, table("MSysQueries", true));
    database.queries.push(SavedQuery {
        object_id: 2,
        name: "Created".into(),
        system: false,
    });
    database
}

fn created(storage: QueryCreationCatalogChange<'_>) -> Change<'_> {
    Change::QueryCreated {
        object_id: 2,
        name: "Created",
        storage,
    }
}

#[test]
fn only_the_nominal_initialization_delta_admits_the_single_technical_table() {
    let old = before();
    let mut fresh = after();
    let initialized = created(QueryCreationCatalogChange::InitializedQueries { storage: STORAGE });
    assert!(matches_change(&old, &fresh, initialized));
    assert!(!matches_change(
        &old,
        &fresh,
        created(QueryCreationCatalogChange::ExistingTables)
    ));
    assert!(!matches(&old, &fresh));

    fresh.tables.remove(1);
    assert!(matches_change(
        &old,
        &fresh,
        created(QueryCreationCatalogChange::ExistingTables)
    ));
    assert!(!matches_change(&old, &fresh, initialized));
}

#[test]
fn initialized_role_must_match_its_validated_descriptor_without_any_other_table_delta() {
    let old = before();
    let initialized = created(QueryCreationCatalogChange::InitializedQueries { storage: STORAGE });
    for case in 0..8 {
        let mut fresh = after();
        match case {
            0 => fresh.tables[1].system = false,
            1 => fresh.tables[1].local = false,
            2 => fresh.tables[1].storage = "foreign descriptor".into(),
            3 => fresh.tables.push(table("Extra", false)),
            4 => fresh.tables[0].name = "Renamed".into(),
            5 => fresh.tables.swap(0, 2),
            6 => fresh.tables[2].storage = "changed old storage".into(),
            7 => fresh.tables[1].name = "DifferentSystemTable".into(),
            _ => unreachable!(),
        }
        assert!(!matches_change(&old, &fresh, initialized), "case {case}");
    }

    let mut already_present = before();
    already_present.tables.push(table("MSysQueries", true));
    assert!(!matches_change(&already_present, &after(), initialized));
}

#[test]
fn technical_initialization_preserves_every_prior_query_and_exact_created_identity() {
    let old = before();
    let initialized = created(QueryCreationCatalogChange::InitializedQueries { storage: STORAGE });
    for case in 0..5 {
        let mut fresh = after();
        match case {
            0 => fresh.queries[0].object_id = 77,
            1 => fresh.queries[0].name = "Replaced".into(),
            2 => fresh.queries[1].system = true,
            3 => fresh.queries[1].object_id = 3,
            4 => fresh.queries.push(SavedQuery {
                object_id: 3,
                name: "ExtraQuery".into(),
                system: false,
            }),
            _ => unreachable!(),
        }
        assert!(!matches_change(&old, &fresh, initialized), "case {case}");
    }
}
