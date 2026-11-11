use crate::{
    permissions::{Permissions, access},
    requests,
};
use rmcp::model::Tool;

// Names are protocol surface, not alternate implementations of these operations.
const OPERATIONS: &[(&str, &str)] = &[
    (
        "retry_unchanged_field_rename",
        "Retry only the accepted complete-image read for an unchanged field rename; never start a native save.",
    ),
    (
        "discard_unchanged_field_rename",
        "Discard the actual retained unchanged-rename read context without writing native data.",
    ),
    (
        "retry_insert",
        "Verify the retained complete original image and obtain a fresh insertion draft; never replay the old review or inputs.",
    ),
    (
        "discard_insert_retry",
        "Discard only the acknowledged insertion retry context; never roll back or release an active native owner.",
    ),
    (
        "session_status",
        "Discover the current revision and actual save/creation owner IDs after a lost response.",
    ),
    (
        "describe_field_filter",
        "Inspect actual native field filter operators.",
    ),
    (
        "apply_field_filter",
        "Apply a checked native field filter, retaining its real handle.",
    ),
    (
        "filter_selection",
        "Create a filter from one actual native row cell.",
    ),
    (
        "filter_page",
        "Read one page through a retained filter handle.",
    ),
    (
        "clear_filter",
        "Clear criteria on the retained view while preserving its sorting.",
    ),
    (
        "reapply_filter",
        "Revalidate a retained inert filter definition against the current source.",
    ),
    (
        "sort_view",
        "Sort a native view through the shared runtime.",
    ),
    (
        "describe_totals",
        "Inspect supported totals for this native view.",
    ),
    (
        "calculate_totals",
        "Calculate checked totals over the selected native view.",
    ),
    (
        "describe_query_design",
        "Inspect actual native metadata for visual query construction.",
    ),
    (
        "import_query_design",
        "Import SQL into the shared checked visual query model.",
    ),
    (
        "build_query_design",
        "Build checked SQL for a headless visual query draft against native metadata.",
    ),
    (
        "rebuild_query_design",
        "Rebuild the exact retained visual query draft.",
    ),
    (
        "find_next",
        "Find an unformatted occurrence and retain its actual runtime position.",
    ),
    (
        "continue_find",
        "Continue from the exact retained occurrence, consuming its find handle.",
    ),
    (
        "replace_found",
        "Create an edit draft for the exact retained occurrence; review and apply separately.",
    ),
    (
        "open_database",
        "Open a stable MDB/ACCDB using shared runtime limits.",
    ),
    (
        "list_tables",
        "Return the current published catalog and document revision.",
    ),
    ("read_rows", "Read one bounded page from a local table."),
    (
        "describe_table",
        "Inspect native field metadata without editing.",
    ),
    (
        "inspect_collection",
        "Inspect a stored collection for one native parent row.",
    ),
    (
        "inspect_query",
        "Inspect SQL and retain its runtime-owned query handle.",
    ),
    (
        "run_query",
        "Execute a retained inspected read query with explicit parameters and bounded per-run options.",
    ),
    (
        "inspect_saved_query",
        "Inspect a saved query by exact published object identity.",
    ),
    (
        "run_saved_query",
        "Execute an explicitly selected saved read query with bounded per-run options.",
    ),
    ("begin_edit", "Obtain a native cell edit draft."),
    (
        "review_edit",
        "Review literal edit input; retain the sealed native capability.",
    ),
    (
        "begin_insert",
        "Obtain an insertion draft in native field order.",
    ),
    (
        "review_insert",
        "Review an insertion, preserving omitted fields separately from NULL.",
    ),
    (
        "review_delete",
        "Review deletion of one snapshot-local row.",
    ),
    (
        "review_table_rename",
        "Review an exact native table rename.",
    ),
    (
        "review_table_creation",
        "Review a native table draft through the shared builder.",
    ),
    (
        "inspect_table_drop",
        "Inspect eligibility for native table deletion.",
    ),
    (
        "review_table_drop",
        "Review qualified empty-table deletion; broader DROP remains refused.",
    ),
    (
        "inspect_field_addition",
        "Inspect qualified native field addition availability.",
    ),
    (
        "review_field_addition",
        "Review one validated saved-field gallery declaration with exact type parameters.",
    ),
    (
        "inspect_field_rename",
        "Inspect an exact native field rename target.",
    ),
    (
        "review_field_rename",
        "Review an exact native field rename.",
    ),
    (
        "inspect_field_drop",
        "Inspect an exact native field deletion target.",
    ),
    (
        "review_field_drop",
        "Review qualified native field deletion.",
    ),
    (
        "inspect_database_options",
        "Inspect native Name AutoCorrect state.",
    ),
    (
        "review_disable_name_auto_correct",
        "Review disabling native Name AutoCorrect.",
    ),
    (
        "inspect_query_sql",
        "Read the editable SQL draft of an exact saved query.",
    ),
    (
        "review_query_sql",
        "Review a saved-query SQL replacement against its original text.",
    ),
    (
        "review_query_expression",
        "Review a qualified literal expression replacement.",
    ),
    ("review_query_insert", "Review an inspected INSERT query."),
    ("review_query_delete", "Review an inspected DELETE query."),
    ("review_query_update", "Review an inspected UPDATE query."),
    (
        "create_database",
        "Start exclusive creation of a new ACE16 database; never overwrite.",
    ),
    (
        "creation_status",
        "Read the actual retained creation outcome.",
    ),
    (
        "cancel_creation",
        "Request cooperative creation cancellation; do not assume rollback.",
    ),
    (
        "acknowledge_creation",
        "Acknowledge a terminal creation report.",
    ),
    (
        "open_created_database",
        "Open only the exact runtime-qualified created image.",
    ),
    (
        "start_reserved_recovery",
        "Start explicit recovery of a checked journal beside an authorized source.",
    ),
    (
        "save_status",
        "Read the actual retained native save/recovery report.",
    ),
    (
        "cancel_save",
        "Request cooperative save cancellation; a write may already have committed.",
    ),
    (
        "recover_save",
        "Recover the retained checked journal using its actual resource owner.",
    ),
    (
        "release_save",
        "Release retained exclusive resources through their owning worker.",
    ),
    (
        "acknowledge_save",
        "Acknowledge the terminal report and perform its guarded reopening.",
    ),
];

const APPLY_KINDS: &[&str] = &[
    "edit",
    "insert",
    "delete",
    "table_rename",
    "table_creation",
    "table_drop",
    "field_addition",
    "field_rename",
    "field_drop",
    "database_options",
    "query_sql",
    "query_expression",
    "query_insert",
    "query_delete",
    "query_update",
];

pub(super) fn definition(name: &str, permissions: Permissions) -> Option<Tool> {
    if !permissions.allows(name) {
        return None;
    }
    if let Some(kind) = name.strip_prefix("apply_")
        && APPLY_KINDS.contains(&kind)
    {
        return apply_tool(name.to_owned());
    }
    OPERATIONS
        .iter()
        .find(|(operation, _)| *operation == name)
        .and_then(|(name, description)| tool((*name).to_owned(), description))
}

pub(super) fn known(name: &str) -> bool {
    OPERATIONS.iter().any(|(operation, _)| *operation == name)
        || name
            .strip_prefix("apply_")
            .is_some_and(|kind| APPLY_KINDS.contains(&kind))
}

pub(super) fn definitions(permissions: Permissions) -> Result<Vec<Tool>, &'static str> {
    if OPERATIONS.iter().any(|(name, _)| access(name).is_none())
        || APPLY_KINDS
            .iter()
            .any(|kind| access(&format!("apply_{kind}")).is_none())
    {
        return Err("registered tool has no typed access classification");
    }

    let operations = OPERATIONS
        .iter()
        .filter(|(name, _)| permissions.allows(name))
        .map(|(name, description)| tool((*name).to_owned(), description));
    let applies = APPLY_KINDS
        .iter()
        .map(|kind| format!("apply_{kind}"))
        .filter(|name| permissions.allows(name))
        .map(apply_tool);
    operations
        .chain(applies)
        .map(|tool| tool.ok_or("registered tool has no request schema"))
        .collect()
}

fn tool(name: String, description: &'static str) -> Option<Tool> {
    requests::schema(&name).map(|schema| Tool::new(name, description, schema))
}

fn apply_tool(name: String) -> Option<Tool> {
    let description = if name == "apply_field_rename" {
        "Consume the exact review. An unchanged name performs only complete-image verification; a changed name starts a native save. Inspect the returned operation_id or save_id."
    } else {
        "Consume the exact retained review and start its native save. Poll save_status and acknowledge the actual outcome."
    };
    tool(name, description)
}
