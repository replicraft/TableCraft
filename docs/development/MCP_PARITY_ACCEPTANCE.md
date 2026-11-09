# Desktop-to-MCP capability acceptance docket

This inventory checks the actual current `Action` enum, all runtime `Command`
variants, the normal desktop dispatcher and the MCP session implementation. It
is intentionally more specific than tool counts or matching API names. There are
100 current shared-UI actions and 60 runtime command variants; every name appears
once in its table below. Feature-gated legacy primitives are identified rather
than silently counted as active desktop functionality.

## Evidence states

- **S0**: functional host cases already observed in the earlier local MCP batch,
  including actual files, scoped protocol/actor checks and stdio process execution.
- **S1**: new source-qualified MCP acceptance cases added in
  `apps/tablecraft-mcp/src/session/acceptance/`; these require the consolidated
  batch before claiming a pass. They reuse existing independent raw builders and
  private Session helpers instead of copying native qualification suites.
- **Presentation-only**: the action owns widgets, tabs, hidden raw input or native
  file pickers; it grants no backend authority and does not require a widget API
  in MCP. The corresponding actual semantic operations are still listed.

A status for a workflow does not establish every parameter/profile combination.
Windows native filesystem execution and Microsoft Access behavioral comparisons
remain separate. Full arbitrary CRUD/DDL, populated-table DROP, relationships,
forms/reports and deferred VBA are not supplied by current source qualification.

## Concrete functional acceptance

| Family | Actual observation / acceptance case | Boundary still explicit |
| --- | --- | --- |
| Row edit/insert/delete | `session/tests.rs`: real files, actual consuming capabilities, fresh revision/membership | Snapshot-local ordinals, omitted/NULL/value semantics and native restrictions |
| Action SQL | `acceptance/action_queries.rs`: parameterized INSERT default-vs-NULL, UPDATE independent complete AFTER, DELETE remaining 62-row membership | Review never executes twice at apply; wrong operation and stale handles refuse |
| Table/field schema | `acceptance/schema.rs`: exact field ADD/RENAME and empty-table catalog DROP AFTERs | Metadata eligibility is not save authority; no allocated-page reclamation claim |
| Table builders | `acceptance/builder_properties.rs` plus builder input checks | Same design no-key confirmation and collision-safe ID1; generated ID remains protected |
| Options and saved queries | Existing full-SQL case plus `acceptance/other_services.rs` | Exact options/literal AFTER, separate actual saved read; no VBA execution |
| Read-side stored collections | `acceptance/other_services.rs`: multivalue, attachment metadata, version history | Encoded metadata is not an executable attachment action |
| Find/filter/sort/totals/design | Feature-local actual Session cases in `auxiliary.rs`, `filtering.rs`, `query_design.rs` | Real handles/metadata; no invented criteria operands or hidden execution |
| Recovery/cancellation | `acceptance/recovery.rs`, actor and cancellation tests | Real reserved journals/permissions/ID chains; cancellation does not imply rollback |
| Failed INSERT continuation | `acceptance/insertion_retry.rs` | Actual witness/evidence, verified original, fresh draft and new explicit review; old input is not replayed |

## Table-builder property accounting

The MCP accepts a complete checked draft rather than recreating GUI editor state.
Each field uses the same `TableDraft` setters and final snapshot as the desktop.

| Current desktop behavior | MCP representation and acceptance | What is not claimed |
| --- | --- | --- |
| Initial protected incrementing ID | `initial_autonumber: true`; normal real table-creation case | No generated-ID rename/type alteration API; the desktop also protects it |
| Manual Design with optional primary | `initial_autonumber: false`, per-field `primary` through checked builder | No arbitrary native index editing |
| No-key confirmation Yes | `add_incrementing_primary_key: true`; actual design ID collision produces ID1 and fresh AutoNumber metadata | Contradictory existing/manual keys refuse |
| Field type and Number size | Explicit `kind` / `number_size` values accepted by the shared builder | MCP does not newly qualify every native type/profile pair |
| Decimal declaration | Paired `precision`/`scale`, shared `DecimalConstraints`; half declaration refuses | No implicit rounding/coercion or ignored scale |
| Text width / required / zero length | Checked width and optional declaration properties; real design save/reopen observes width32/false/true | No conversion of reader-nullability into Required |
| Defaults | TypeDefault, Absent, Null, Literal modes; checked input tests and actual absent default observation | Hidden GUI text caches are not persisted semantics; conflicting literal input refuses |
| Description / names | Exact supplied names/descriptions via shared setters; real property reread | Captions are not native field identity; no unsupported existing-field property ALTER |
| Add/remove/reorder unsaved fields | Complete ordered `fields` input describes final unsaved schema | No native row exists until separate insertion review/apply |
| Pending raw record alongside unsaved schema | Create schema, guarded reopen, then fresh insertion draft/review/apply | No invented atomic schema-plus-record transaction or generated defaults |

## Every shared UI action

| Action | MCP semantic counterpart / ownership | Acceptance |
| --- | --- | --- |
| `InspectFieldDrop` | `inspect_field_drop` / `review_field_drop` | S0: real saved-field DROP; pending mark/confirmation is presentation-owned |
| `ReviewFieldDrop` | `inspect_field_drop` / `review_field_drop` | S0: real saved-field DROP; pending mark/confirmation is presentation-owned |
| `CancelFieldDropInspection` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `CancelFieldDropReview` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `FieldDropSave` | `apply_field_drop` plus save lifecycle | S0: exact independent AFTER and witness reopen |
| `FieldDropClose` | Host presentation dismissal / end of pending input; no native mutation | No identical widget API needed: MCP separates review/apply and can abandon or supersede a bounded review; save owners still require explicit resolution/ACK |
| `InspectFieldRename` | `inspect_field_rename`, `review_field_rename`, `apply_field_rename` | S1: Number/Text true-name changes and native-number preservation |
| `ReviewFieldRename` | `inspect_field_rename`, `review_field_rename`, `apply_field_rename` | S1: Number/Text true-name changes and native-number preservation |
| `CancelFieldRenameReview` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `FieldRenameSave` | `inspect_field_rename`, `review_field_rename`, `apply_field_rename` | S1: Number/Text true-name changes and native-number preservation |
| `FieldRenameClose` | Host presentation dismissal / end of pending input; no native mutation | No identical widget API needed: MCP separates review/apply and can abandon or supersede a bounded review; save owners still require explicit resolution/ACK |
| `CloseFieldRenameEditor` | Host presentation dismissal / end of pending input; no native mutation | No identical widget API needed: MCP separates review/apply and can abandon or supersede a bounded review; save owners still require explicit resolution/ACK |
| `InspectDatabaseOptions` | `inspect_database_options`, `review_disable_name_auto_correct`, `apply_database_options` | S1: explicit-OFF complete-image save/reopen |
| `DisableNameAutoCorrect` | `inspect_database_options`, `review_disable_name_auto_correct`, `apply_database_options` | S1: explicit-OFF complete-image save/reopen |
| `CancelDatabaseOptionsReview` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `DatabaseOptionsSave` | `inspect_database_options`, `review_disable_name_auto_correct`, `apply_database_options` | S1: explicit-OFF complete-image save/reopen |
| `FieldAdditionClose` | Host presentation dismissal / end of pending input; no native mutation | No identical widget API needed: MCP separates review/apply and can abandon or supersede a bounded review; save owners still require explicit resolution/ACK |
| `ReviewFieldAddition` | `inspect_field_addition`, `review_field_addition`, `apply_field_addition` | S1: Number and exact Short Text width/compression |
| `RetryFieldAdditionInspection` | `inspect_field_addition`, `review_field_addition`, `apply_field_addition` | S1: Number and exact Short Text width/compression |
| `CancelFieldAdditionReview` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `FieldAdditionSave` | `inspect_field_addition`, `review_field_addition`, `apply_field_addition` | S1: Number and exact Short Text width/compression |
| `InspectTableDrop` | `inspect_table_drop`, `review_table_drop`, `apply_table_drop` | S1: exact empty-table native ID/count; logical tombstone AFTER |
| `CancelTableDropInspection` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `DeleteTable` | `inspect_table_drop`, `review_table_drop`, `apply_table_drop` | S1: exact empty-table native ID/count; logical tombstone AFTER |
| `CancelTableDropReview` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `TableDropSave` | `inspect_table_drop`, `review_table_drop`, `apply_table_drop` | S1: exact empty-table native ID/count; logical tombstone AFTER |
| `CancelTableRenameReview` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `RenameTable` | `review_table_rename`, `apply_table_rename` | S0: real table rename, independent AFTER and fresh catalog |
| `ApplyQueryAction` | `apply_query_insert`, `apply_query_update`, `apply_query_delete` | S1: actual parameterized native actions, separate consuming reviews |
| `CancelQueryActionReview` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `BeginRecovery` | `start_reserved_recovery`, `recover_save` / `release_save`, `acknowledge_save` | S1: actual reserved pending/committed/malformed journals and ID rebinding |
| `CloseRecovery` | Host presentation dismissal / end of pending input; no native mutation | No identical widget API needed: MCP separates review/apply and can abandon or supersede a bounded review; save owners still require explicit resolution/ACK |
| `CancelRecovery` | `cancel_save` | S1: explicit save cancellation observes actual terminal outcome, never infers rollback |
| `ResolveRecovery` | `start_reserved_recovery`, `recover_save` / `release_save`, `acknowledge_save` | S1: actual reserved pending/committed/malformed journals and ID rebinding |
| `AcknowledgeRecovery` | `start_reserved_recovery`, `recover_save` / `release_save`, `acknowledge_save` | S1: actual reserved pending/committed/malformed journals and ID rebinding |
| `SaveTableDraft` | `review_table_creation`, `apply_table_creation` | S0+S1: blank-to-table; design no-key generated ID1 and explicit properties |
| `CancelTableDraftSave` | `cancel_save` | S1: explicit save cancellation observes actual terminal outcome, never infers rollback |
| `RecoverTableDraftSave` | `recover_save` | S1: real retained owner permissions; no arbitrary capability reconstruction |
| `ReleaseTableDraftSave` | `release_save` | S1: actual owner and new actor ID; reserved artifact retained when unqualified |
| `AcknowledgeTableDraftSave` | `acknowledge_save` | S0+S1: actual ACK before appropriate reopen; failed full-image read retains witness |
| `SavedQuerySqlSave` | `inspect_query_sql`, `review_query_sql`, `apply_query_sql` | S0: complete saved SQL text, exact independent AFTER and failed-witness retry |
| `SavedQuerySqlClose` | Host presentation dismissal / end of pending input; no native mutation | No identical widget API needed: MCP separates review/apply and can abandon or supersede a bounded review; save owners still require explicit resolution/ACK |
| `TableDraftsDiscarded` | Host-only tabs, drafts, file picker or view transition | No native capability gap: explicit path/page/schema/query commands supply services; unsaved GUI caches/layout are not protocol authority |
| `TableDraftDiscardCancelled` | Host-only tabs, drafts, file picker or view transition | No native capability gap: explicit path/page/schema/query commands supply services; unsaved GUI caches/layout are not protocol authority |
| `ChooseFile` | Host-only tabs, drafts, file picker or view transition | No native capability gap: explicit path/page/schema/query commands supply services; unsaved GUI caches/layout are not protocol authority |
| `ChooseCreationLocation` | Host-only tabs, drafts, file picker or view transition | No native capability gap: explicit path/page/schema/query commands supply services; unsaved GUI caches/layout are not protocol authority |
| `CreateDatabase` | `create_database`, creation status/cancel/ACK, `open_created_database` | S0: actual separate publication worker; exact canonical created-byte open |
| `Open` | `open_database` | S0: actual source/captured revision; actor root and input guards |
| `Page` | `read_rows` / `filter_page` with explicit offset | S0: real bounded pages and native row ordinals; navigator widgets are presentation |
| `ReviewSavedQuerySql` | `inspect_query_sql`, `review_query_sql`, `apply_query_sql` | S0: complete saved SQL text, exact independent AFTER and failed-witness retry |
| `InspectSavedQuery` | `inspect_saved_query`, `run_saved_query` | S0+S1: exact object identity; saved SQL/literal result explicitly executed |
| `RunParameterizedQuery` | `inspect_query`, `run_query`, action-specific review when classified | S0+S1: source-free/read/parameterized action paths; inspect is not execution |
| `RunSavedQuery` | `inspect_saved_query`, `run_saved_query` | S0+S1: exact object identity; saved SQL/literal result explicitly executed |
| `NavigateRecord` | `read_rows` / `filter_page` with explicit offset | S0: real bounded pages and native row ordinals; navigator widgets are presentation |
| `ViewSchema` | `describe_table` | S0+S1: actual field metadata, native numbers and properties |
| `ViewContents` | `inspect_collection` | S1: multivalue, attachment metadata and version-history kinds; no executable attachment action |
| `CloseContents` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `OpenObject` | Host-only tabs, drafts, file picker or view transition | No native capability gap: explicit path/page/schema/query commands supply services; unsaved GUI caches/layout are not protocol authority |
| `ActivateObject` | Host-only tabs, drafts, file picker or view transition | No native capability gap: explicit path/page/schema/query commands supply services; unsaved GUI caches/layout are not protocol authority |
| `CloseObject` | Host-only tabs, drafts, file picker or view transition | No native capability gap: explicit path/page/schema/query commands supply services; unsaved GUI caches/layout are not protocol authority |
| `RetryObject` | Host-only tabs, drafts, file picker or view transition | No native capability gap: explicit path/page/schema/query commands supply services; unsaved GUI caches/layout are not protocol authority |
| `ViewObjectDatasheet` | Host-only tabs, drafts, file picker or view transition | No native capability gap: explicit path/page/schema/query commands supply services; unsaved GUI caches/layout are not protocol authority |
| `SortView` | `sort_view` | S0: actual filter handle, sorted original ordinals and source checks |
| `BeginEdit` | `begin_edit`, literal `review_edit`, `apply_edit` | S0: real cell update; clipboard acquisition is host-specific, text uses same parser |
| `PasteCell` | `begin_edit`, literal `review_edit`, `apply_edit` | S0: real cell update; clipboard acquisition is host-specific, text uses same parser |
| `DescribeTotals` | `describe_totals`, `calculate_totals` | S0: real complete filtered-view sum; aggregate kernels remain runtime-qualified |
| `ComputeTotals` | `describe_totals`, `calculate_totals` | S0: real complete filtered-view sum; aggregate kernels remain runtime-qualified |
| `CancelTotals` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `DescribeFieldFilter` | `describe_field_filter`, `apply_field_filter`, `filter_selection`, `filter_page`, `clear_filter` | S0: real native membership, sort/off/clear and stale/wrong-table refusals |
| `ApplyFieldFilter` | `describe_field_filter`, `apply_field_filter`, `filter_selection`, `filter_page`, `clear_filter` | S0: real native membership, sort/off/clear and stale/wrong-table refusals |
| `CloseFieldFilter` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `FilterSelection` | `describe_field_filter`, `apply_field_filter`, `filter_selection`, `filter_page`, `clear_filter` | S0: real native membership, sort/off/clear and stale/wrong-table refusals |
| `FilterPage` | `describe_field_filter`, `apply_field_filter`, `filter_selection`, `filter_page`, `clear_filter` | S0: real native membership, sort/off/clear and stale/wrong-table refusals |
| `ClearTableFilter` | `describe_field_filter`, `apply_field_filter`, `filter_selection`, `filter_page`, `clear_filter` | S0: real native membership, sort/off/clear and stale/wrong-table refusals |
| `ReplaceFindMatch` | `find_next`, `continue_find`, `replace_found` | S0: actual found occurrence -> normal edit draft/review/save; formatted/filtered cases added separately |
| `FindNext` | `find_next`, `continue_find`, `replace_found` | S0: actual found occurrence -> normal edit draft/review/save; formatted/filtered cases added separately |
| `CancelFind` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `ReviewDelete` | `review_delete`, `apply_delete` | S0: real row removal and fresh membership |
| `ApplyDelete` | `review_delete`, `apply_delete` | S0: real row removal and fresh membership |
| `CloseDelete` | Host presentation dismissal / end of pending input; no native mutation | No identical widget API needed: MCP separates review/apply and can abandon or supersede a bounded review; save owners still require explicit resolution/ACK |
| `BeginInsert` | `begin_insert`, `review_insert`, `apply_insert` | S0: actual field order, presence-bearing input and native insertion |
| `ReviewInsert` | `begin_insert`, `review_insert`, `apply_insert` | S0: actual field order, presence-bearing input and native insertion |
| `ApplyInsert` | `begin_insert`, `review_insert`, `apply_insert` | S0: actual field order, presence-bearing input and native insertion |
| `CloseInsert` | Host presentation dismissal / end of pending input; no native mutation | No identical widget API needed: MCP separates review/apply and can abandon or supersede a bounded review; save owners still require explicit resolution/ACK |
| `ReviewEdit` | `begin_edit`, literal `review_edit`, `apply_edit` | S0: real cell update; clipboard acquisition is host-specific, text uses same parser |
| `ApplyEdit` | `begin_edit`, literal `review_edit`, `apply_edit` | S0: real cell update; clipboard acquisition is host-specific, text uses same parser |
| `CloseEdit` | Host presentation dismissal / end of pending input; no native mutation | No identical widget API needed: MCP separates review/apply and can abandon or supersede a bounded review; save owners still require explicit resolution/ACK |
| `CancelEditSave` | `cancel_save` | S1: explicit save cancellation observes actual terminal outcome, never infers rollback |
| `RecoverEditSave` | `recover_save` | S1: real retained owner permissions; no arbitrary capability reconstruction |
| `ReleaseEditSave` | `release_save` | S1: actual owner and new actor ID; reserved artifact retained when unqualified |
| `RetryInsert` | `retry_insert` | S1: actual no-write failure evidence; full original check -> fresh draft/new review, never old-input replay |
| `CancelInsertRetry` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `DiscardRetainedInsert` | `discard_insert_retry` | S1: explicit advisory-witness discard after real ACK, unchanged file |
| `AcknowledgeEditSave` | `acknowledge_save` | S0+S1: actual ACK before appropriate reopen; failed full-image read retains witness |
| `DescribeQueryDesign` | `describe_query_design`, `build_query_design`, `import_query_design`, `rebuild_query_design` | S0: native metadata, checked import/build/rebuild and distinct explicit run |
| `BuildQueryDesign` | `describe_query_design`, `build_query_design`, `import_query_design`, `rebuild_query_design` | S0: native metadata, checked import/build/rebuild and distinct explicit run |
| `ImportQueryDesign` | `describe_query_design`, `build_query_design`, `import_query_design`, `rebuild_query_design` | S0: native metadata, checked import/build/rebuild and distinct explicit run |
| `CancelQueryDesign` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |
| `Query` | `inspect_query`, `run_query`, action-specific review when classified | S0+S1: source-free/read/parameterized action paths; inspect is not execution |
| `Cancel` | SDK request cancellation; explicit save/creation cancel when those actors are active | S0: accepted-read and post-ACK cancellation; exact individual dialog races are not all separately repeated |

## Every runtime command

Private witness commands are deliberately not JSON tool parameters: only an
actual accepted runtime capability/report can supply those witnesses.

| Command | MCP route / active desktop distinction | Acceptance |
| --- | --- | --- |
| `InspectTableDropSource` | Table-DROP tools and private exact witness reopen | S1: schema acceptance |
| `ReviewTableDrop` | Table-DROP tools and private exact witness reopen | S1: schema acceptance |
| `OpenTableDropped` | Table-DROP tools and private exact witness reopen | S1: schema acceptance |
| `InspectDatabaseOptions` | Database-options tools and private exact witness reopen | S1: other-services acceptance |
| `ReviewDisableNameAutoCorrect` | Database-options tools and private exact witness reopen | S1: other-services acceptance |
| `OpenDatabaseOptionsSaved` | Database-options tools and private exact witness reopen | S1: other-services acceptance |
| `ReviewTableRename` | Table-rename review/apply and private exact witness reopen | S0: independent complete AFTER |
| `InspectQuerySqlDraft` | Saved-SQL tools; actual witness also serves singular expression edits | S0+S1: complete SQL and distinct literal edit |
| `ReviewQuerySql` | Saved-SQL tools; actual witness also serves singular expression edits | S0+S1: complete SQL and distinct literal edit |
| `ReviewQueryLiteralExpression` | Additional `review_query_expression` / `apply_query_expression` service | S1: real scalar-literal save; normal desktop SQL editor uses complete-SQL route |
| `OpenQueryExpressionSaved` | Saved-SQL tools; actual witness also serves singular expression edits | S0+S1: complete SQL and distinct literal edit |
| `OpenTableRenamed` | Table-rename review/apply and private exact witness reopen | S0: independent complete AFTER |
| `OpenInsertOriginal` | Private witness command used only by `retry_insert` | S1: source mismatch refusal, full original reopen, fresh inputs/review |
| `ReviewFieldDrop` | Field-DROP tools and private exact witness reopen | S0: real Number DROP and exact after |
| `OpenFieldDropped` | Field-DROP tools and private exact witness reopen | S0: real Number DROP and exact after |
| `InspectFieldDropSource` | Field-DROP tools and private exact witness reopen | S0: real Number DROP and exact after |
| `ReviewFieldRename` | Field-rename tools and private exact witness reopen | S1: real Number/Text after images |
| `OpenFieldRenamed` | Field-rename tools and private exact witness reopen | S1: real Number/Text after images |
| `InspectFieldRenameSource` | Field-rename tools and private exact witness reopen | S1: real Number/Text after images |
| `InspectFieldAdditionAvailability` | Generic saved-field gallery tools and private exact witness reopen | S1: Number and explicit Short Text |
| `ReviewAddField` | Generic saved-field gallery tools and private exact witness reopen | S1: Number and explicit Short Text |
| `InspectNumberColumnAvailability` | Retained low-level runtime primitives; not called by active normal desktop | Intentionally unexposed: do not confuse generic gallery coverage with these distinct legacy capabilities/profiles |
| `ReviewAddNumberColumn` | Retained low-level runtime primitives; not called by active normal desktop | Intentionally unexposed: do not confuse generic gallery coverage with these distinct legacy capabilities/profiles |
| `ReviewAddInt32Column` | Retained low-level runtime primitives; not called by active normal desktop | Intentionally unexposed: do not confuse generic gallery coverage with these distinct legacy capabilities/profiles |
| `OpenColumnAdded` | Generic saved-field gallery tools and private exact witness reopen | S1: Number and explicit Short Text |
| `ReviewTableCreation` | Table creation through checked builder snapshot + exact witness | S0+S1: normal initial and design collision-safe primary workflows |
| `OpenTableCreated` | Table creation through checked builder snapshot + exact witness | S0+S1: normal initial and design collision-safe primary workflows |
| `OpenCreated` | Private retained CreationReport command | S0: real publication/ACK/canonical reopen; JSON cannot author a report |
| `ReplaceFindMatch` | Find/continuation/replace tools retain actual descriptor and view | S0: actual edit continuation; formatted/filtered additions recorded separately |
| `DescribeQueryDesign` | Headless design tools | S0: actual native metadata and checked SQL; no implicit execution |
| `BuildQueryDesign` | Headless design tools | S0: actual native metadata and checked SQL; no implicit execution |
| `ImportQueryDesign` | Headless design tools | S0: actual native metadata and checked SQL; no implicit execution |
| `DescribeTotals` | Totals describe/calculate tools | S0: actual complete filtered view; not all aggregate/profile combinations repeated in MCP |
| `Totals` | Totals describe/calculate tools | S0: actual complete filtered view; not all aggregate/profile combinations repeated in MCP |
| `Open` | Explicit `open_database`; also guarded known row-save/restart refresh | S0+S1: paths/root/revision, creation and recovery branches |
| `BeginEdit` | Cell edit tools | S0: actual update, before/after, stale/wrong-kind/consumed refusal |
| `ReviewEdit` | Cell edit tools | S0: actual update, before/after, stale/wrong-kind/consumed refusal |
| `BeginInsert` | Presence-bearing insertion tools | S0+S1: ordinary insertion and original-image continuation |
| `ReviewInsert` | Presence-bearing insertion tools | S0+S1: ordinary insertion and original-image continuation |
| `ReviewDelete` | Row deletion review/apply | S0: actual removal and fresh membership |
| `SortView` | Native view tools | S0: actual sort/filter/off/clear, stale handles and post-write fresh definition reapply |
| `ClearFilter` | Native view tools | S0: actual sort/filter/off/clear, stale handles and post-write fresh definition reapply |
| `ReapplyFilter` | Native view tools | S0: actual sort/filter/off/clear, stale handles and post-write fresh definition reapply |
| `DescribeFieldFilter` | Native view tools | S0: actual sort/filter/off/clear, stale handles and post-write fresh definition reapply |
| `ApplyFieldFilter` | Native view tools | S0: actual sort/filter/off/clear, stale handles and post-write fresh definition reapply |
| `FilterSelection` | Native view tools | S0: actual sort/filter/off/clear, stale handles and post-write fresh definition reapply |
| `FilterPage` | Native view tools | S0: actual sort/filter/off/clear, stale handles and post-write fresh definition reapply |
| `FindFiltered` | Find/continuation/replace tools retain actual descriptor and view | S0: actual edit continuation; formatted/filtered additions recorded separately |
| `FindNext` | Find/continuation/replace tools retain actual descriptor and view | S0: actual edit continuation; formatted/filtered additions recorded separately |
| `Page` | Page/schema/collection tools | S0+S1: actual metadata/properties and three stored collection kinds |
| `Describe` | Page/schema/collection tools | S0+S1: actual metadata/properties and three stored collection kinds |
| `InspectCollection` | Page/schema/collection tools | S0+S1: actual metadata/properties and three stored collection kinds |
| `Query` | Active desktop direct read (including checked design execution); MCP uses inspect/run sequence | S0: explicit checked design result executed through same runtime read semantics; no direct-query bypass tool |
| `InspectQuery` | Inspected/parameterized query tools | S0: retained opaque query ID, parameters and stale refusal |
| `RunQuery` | Inspected/parameterized query tools | S0: retained opaque query ID, parameters and stale refusal |
| `ReviewQueryInsert` | Action-specific typed review/apply tools | S1: actual parameterized native INSERT/DELETE/UPDATE |
| `ReviewQueryDelete` | Action-specific typed review/apply tools | S1: actual parameterized native INSERT/DELETE/UPDATE |
| `ReviewQueryUpdate` | Action-specific typed review/apply tools | S1: actual parameterized native INSERT/DELETE/UPDATE |
| `InspectSavedQuery` | Saved read tools | S0+S1: exact native identity, explicit execution |
| `RunSavedQuery` | Saved read tools | S0+S1: exact native identity, explicit execution |

## Remaining acceptance limits

The new S1 cases must pass the final batch; merely adding them is not evidence.
Individual cancellation timings, every aggregate/type combination, every query
shape and every supported file profile are not duplicated in the MCP suite.
Their detailed native/runtime suites remain their owning evidence. Disabled
Ribbon commands and unsupported SQL/engine operations remain unavailable rather
than acquiring fake success responses in an agent host.

Current full product gaps remain in [the Access roadmap](../features/ACCESS_PARITY_ROADMAP.md).
See [crate ownership](CRATE_OWNERSHIP_REVIEW.md), [the runtime contract](../application/RUNTIME.md)
and [the MCP interface](../interfaces/MCP.md) for the shared boundaries.
