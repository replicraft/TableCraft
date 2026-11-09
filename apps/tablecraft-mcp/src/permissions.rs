//! Trusted process-start grants; database content and tool arguments cannot change them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Permissions {
    pub data_write: bool,
    pub schema_write: bool,
    pub application_write: bool,
    pub maintenance: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Access {
    Read,
    DataWrite,
    SchemaWrite,
    ApplicationWrite,
    Maintenance,
    /// This starts no new authority; Session also requires its actual retained owner.
    OwnedLifecycle,
}

impl Permissions {
    pub(super) fn allows(self, operation: &str) -> bool {
        match access(operation) {
            Some(Access::Read | Access::OwnedLifecycle) => true,
            Some(Access::DataWrite) => self.data_write,
            Some(Access::SchemaWrite) => self.schema_write,
            Some(Access::ApplicationWrite) => self.application_write,
            Some(Access::Maintenance) => self.maintenance,
            None => false,
        }
    }

    pub(super) fn require(self, operation: &str) -> Result<(), String> {
        if self.allows(operation) {
            return Ok(());
        }
        let flag = match access(operation) {
            Some(Access::DataWrite) => "--allow-data-write",
            Some(Access::SchemaWrite) => "--allow-schema-write",
            Some(Access::ApplicationWrite) => "--allow-application-write",
            Some(Access::Maintenance) => "--allow-maintenance",
            _ => return Err("Unknown or unclassified TableCraft operation".into()),
        };
        Err(format!(
            "This operation requires the trusted startup grant {flag}; tool arguments cannot enable it"
        ))
    }

    #[cfg(test)]
    pub(super) fn all() -> Self {
        Self {
            data_write: true,
            schema_write: true,
            application_write: true,
            maintenance: true,
        }
    }
}

pub(super) fn access(operation: &str) -> Option<Access> {
    Some(match operation {
        "apply_edit" | "apply_insert" | "apply_delete" | "apply_query_insert"
        | "apply_query_update" | "apply_query_delete" => Access::DataWrite,
        "apply_table_rename"
        | "apply_table_creation"
        | "apply_table_drop"
        | "apply_field_addition"
        | "apply_field_rename"
        | "apply_field_drop"
        | "apply_database_options"
        | "create_database" => Access::SchemaWrite,
        "apply_query_sql" | "apply_query_expression" => Access::ApplicationWrite,
        "start_reserved_recovery" => Access::Maintenance,
        "save_status"
        | "cancel_save"
        | "recover_save"
        | "release_save"
        | "acknowledge_save"
        | "creation_status"
        | "cancel_creation"
        | "acknowledge_creation"
        | "open_created_database"
        | "retry_insert"
        | "retry_unchanged_field_rename"
        | "discard_unchanged_field_rename"
        | "discard_insert_retry" => Access::OwnedLifecycle,
        "session_status"
        | "open_database"
        | "list_tables"
        | "read_rows"
        | "describe_table"
        | "inspect_collection"
        | "inspect_query"
        | "run_query"
        | "inspect_saved_query"
        | "run_saved_query"
        | "begin_edit"
        | "review_edit"
        | "begin_insert"
        | "review_insert"
        | "review_delete"
        | "review_table_rename"
        | "review_table_creation"
        | "inspect_table_drop"
        | "review_table_drop"
        | "inspect_field_addition"
        | "review_field_addition"
        | "inspect_field_rename"
        | "review_field_rename"
        | "inspect_field_drop"
        | "review_field_drop"
        | "inspect_database_options"
        | "review_disable_name_auto_correct"
        | "inspect_query_sql"
        | "review_query_sql"
        | "review_query_expression"
        | "review_query_insert"
        | "review_query_delete"
        | "review_query_update"
        | "describe_field_filter"
        | "apply_field_filter"
        | "filter_selection"
        | "filter_page"
        | "clear_filter"
        | "reapply_filter"
        | "sort_view"
        | "describe_totals"
        | "calculate_totals"
        | "describe_query_design"
        | "import_query_design"
        | "build_query_design"
        | "rebuild_query_design"
        | "find_next"
        | "continue_find"
        | "replace_found" => Access::Read,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_tool_has_typed_access_and_default_mode_hides_new_physical_writes() {
        let all = crate::tools::definitions(Permissions::all()).unwrap();
        let readonly = crate::tools::definitions(Permissions::default()).unwrap();
        for tool in all {
            let classification =
                access(&tool.name).expect("every registered tool must be classified");
            let advertised = readonly.iter().any(|visible| visible.name == tool.name);
            assert_eq!(
                advertised,
                matches!(classification, Access::Read | Access::OwnedLifecycle),
                "{}",
                tool.name
            );
        }
        assert!(
            Permissions::default().allows("apply_field_filter"),
            "filter application does not write native bytes"
        );
        assert!(
            Permissions::default().allows("review_query_delete"),
            "typed review is inert"
        );
        assert!(!Permissions::default().allows("apply_query_delete"));
        assert!(!Permissions::default().allows("apply_unknown_operation"));
    }

    #[test]
    fn each_trusted_grant_is_independent_and_owned_lifecycle_grants_no_new_start() {
        for (permissions, allowed, refused) in [
            (
                Permissions {
                    data_write: true,
                    ..Permissions::default()
                },
                "apply_insert",
                "create_database",
            ),
            (
                Permissions {
                    schema_write: true,
                    ..Permissions::default()
                },
                "create_database",
                "apply_insert",
            ),
            (
                Permissions {
                    application_write: true,
                    ..Permissions::default()
                },
                "apply_query_sql",
                "apply_table_drop",
            ),
            (
                Permissions {
                    maintenance: true,
                    ..Permissions::default()
                },
                "start_reserved_recovery",
                "apply_query_update",
            ),
        ] {
            assert!(permissions.require(allowed).is_ok());
            assert!(permissions.require(refused).is_err());
            assert!(permissions.allows("recover_save"));
        }
    }
}
