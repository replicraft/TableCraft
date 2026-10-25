# Per-table external database references

A SELECT relation can retain an exact external database selector on each named
leaf. This permits local and explicitly mapped external tables in the same
join/product, without rewriting names or moving all FROM sources into one file.

```sh
tablecraft query run --database frontend.accdb \
  --external remote backend.accdb \
  'SELECT l.ID,r.Name FROM LocalPeople l INNER JOIN [remote].[People] r ON l.ID=r.ID'
```

The host opens only files explicitly selected with `--database`, `--link` and
`--external`. Neither a database prefix nor stored native metadata grants
permission to open a path. Existing native format, password, stable/offline and
read-only qualifications remain unchanged.

## Owned syntax and binding

`tablecraft_sql::select::Table::database` is an optional owned String independent
of its Name and alias. Ordinary programmatic tables set it to None. Selectors
preserve exact case, Unicode, paths and punctuation; they are not schema names,
filesystem identities or normalized paths. SQL input uses a bracketed prefix,
`[selector].[Table]`, followed by an optional ordinary local alias. The formatter
doubles closing brackets within selectors, and owned parse/display/parse controls
preserve the exact token. Other quoting styles and three-part database/provider
source forms remain outside this syntax profile. Ordinary table/alias Name rules
are unchanged. Use aliases such as `r.ID` for field lookup; this does not introduce
database-plus-table-plus-field resolution.

Each qualified leaf requests the existing exact operation-root external binding
with no connection syntax. Its captured SourceKey and target metadata become
that leaf's namespace. Missing or differently cased selectors fail without local
fallback. A SELECT-level IN clause still supplies the default namespace for
unqualified leaves; an explicit leaf prefix overrides that default. This
precedence is TableCraft's defined engineering behavior, pending native Access
comparison. Nested reads choose their own FROM context.

Definition ownership remains separate from leaf routing. A selected target saved
query keeps its own lexical namespace for local dependencies, subqueries and
domains. Explicit prefixes within it request only mappings already authorized at
the operation root. Named-query identity, parameter scopes, cycle checks and
execution-stage schema discovery retain their existing scoped keys. Linked
chains, automatic provider activation and recomputed-calculation substitution
remain unsupported.

Prepared external revision keys are revalidated before scans. Existing captured
schema checks still run when each table leaf is reached: a later changed leaf
can be rejected after an earlier unchanged source was read. No rows from the
changed leaf or buffered result escape. This is not all-source schema preflight,
a coherent cross-file snapshot or a guarantee against noncooperating mutation.

## Native saved metadata

Qualified native table rows use attribute 5, Name1 as table name, optional Name2
as alias and Expression as the exact database selector. This representation is
separate from attribute-4 global external metadata and from an expression-only
derived source. Existing field, flag, payload, alias and graph checks remain.
Empty/control-bearing selectors are refused; unknown native layouts stay visible
through inspection and are not guessed into an executable query.

The adapter renders and verifies the existing guarded ordinary relation forest,
then copies selectors directly onto the exact structurally matched owned leaves.
A parsed guard leaf must have no selector before this composition. Stored opaque
strings never become unchecked SQL text. Duplicate table names with distinct
aliases keep their own selectors; no name-only search, synthetic aliases or
filesystem comparison determines association. SELECT, crosstab and guarded UNION
arms use this shared owned-source profile.

Public Jackcess getFromTables observations distinguish per-table prefixes from
global IN and confirm aliases/mixed local joins. Its rendered nested brackets for
punctuated strings are metadata observations, not valid native SQL evidence.
TableCraft's escaped owned serialization is checked separately.

## Execution and editing boundaries

Existing projected/collection callbacks, field policy identities, selected-index
failures, source order and complete-cache boundaries follow the leaf's actual
provider. Root/linked/external native files share one cumulative native allowance;
query, comparison and output pools remain separate. No quota refills on a new
leaf or alias. Required selected-index errors retain their refusal rather than
falling back to a sequential read.

Normal CLI output is buffered. A later source, evaluation, schema, budget or
formatting error publishes no partial result. This does not extend explicit
progressive query admission or authorize external editing. The shared edit
binder refuses qualified table leaves before external resolution, even if a
same-named local table exists and the caller supplies an authorizing resolver.
Parser acceptance of a qualified action-source shape is syntax evidence only.

## References and qualification

Microsoft's [remote-table troubleshooting example](https://learn.microsoft.com/en-us/troubleshoot/microsoft-365-apps/access/operation-is-not-supported-error)
explicitly shows database/table qualification and aliases in Access/ODBC command
text. It establishes the remote-source concept, not TableCraft's bracket escaping,
owned-token authorization, global-IN precedence or native SQL qualification.
The existing [external binding contract](EXTERNAL_QUERY_SOURCES.md) and
[native facade](NATIVE_EXTERNAL_QUERIES.md) define explicit source authorization.
Original public-builder metadata/getters and focused tests establish the current
engineering profile; actual Microsoft Access execution remains separate.
