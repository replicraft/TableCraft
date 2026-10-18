# DAO and ADO object-model compatibility

Status: future behavior requirements, reviewed 18 October 2026. No DAO/ADO host,
database cursor or native source adapter exists. The
[bounded retrieval executor](../QUERY_EXECUTION.md) works over explicit host sources. Core bookmark/locator types
are carriers, not an implementation of either object model.

## Separate observable models

DAO and ADO can share lower-level Automation dispatch contracts, identity,
argument marshalling and supplied database interfaces. Keep their exposed
interfaces, errors, cursor semantics and lifetime rules separate. VBA sees object
contracts, not a concrete Windows provider; see [VBA](../VBA_COMPATIBILITY.md) and
[Windows providers](../WINDOWS_COMPATIBILITY.md). SQL translation alone cannot
reproduce a stateful recordset.

| Surface | Required behavior to qualify |
| --- | --- |
| DAO.Database | Open/close, collections, saved queries, execution results and database identity |
| DAO.TableDef / Field / Index | Types, properties, relationships/index metadata, linked-source metadata and refresh |
| DAO.QueryDef | Ordered/typed parameters, saved/transient definitions, result cursors, affected rows and errors |
| DAO.Workspace | Session/database membership, transaction scope, commit/rollback and supported nesting |
| DAO.Recordset / Recordset2 | Cursor kind, BOF/EOF, navigation, Find/Seek restrictions, bookmarks, filtering, cloning, requery, edit/update/cancel/delete and complex children |
| ADODB.Connection / Command | Provider capabilities, connection state, parameters, execution and transaction contracts |
| ADODB.Recordset | Cursor type/location, requested and actual lock modes, navigation, updates, batch/disconnected behavior and reconnect conflicts |

DAO transaction ownership is workspace-scoped where documented; do not invent an
independent transaction per Database object. External linked providers may have
different rollback capabilities. Model edit buffers separately from committed
records and define what cancellation, validation and required data macros do.
Form event effects are not automatically one rollbackable workspace transaction.

Specify bookmark validity across clones, deletes, updates, requery and close;
do not treat every bookmark as a permanent record ID. Cover empty recordsets,
deleted current records, row visibility, optimistic conflicts, locking and stale
generations. Unsupported cursor methods need the qualified object-model error.

ADO may negotiate a different cursor than requested. Report the actual cursor and
Supports capabilities according to the provider profile; never advertise absent
navigation/update behavior. Distinguish documented negotiation from a fabricated
successful fallback around a denied operation.

## Acceptance evidence

Create native comparison traces for navigation and state transitions, independent
clones, bookmark reuse, edit buffers, cancel/update, query parameters and workspace
rollback across databases. Add linked-provider and ADO cursor/location/lock-mode
matrices, including disconnected updates and concurrent conflicts. Record exact
Access/ACE/provider versions and error numbers. All are unrun requirements; track
application use in the [corpus](../testing/ACCESS_APPLICATIONS.md).

## References

Primary references: [DAO Recordset](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/recordset-object-dao),
[DAO Workspace.BeginTrans](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/workspace-begintrans-method-dao),
[DAO Recordset2](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/recordset2-object-dao),
[ADO Recordset](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/recordset-object-ado)
and [ADO CursorType negotiation](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/cursortype-property-ado).
Method-level implementation needs its specific reference and native comparison.
