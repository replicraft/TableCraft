# Explicit before-event guard preparation

`NativeImage::prepare_ace16_before_event_guards` executes a bounded, explicitly
selected desktop guard subset against original typed INSERT, fixed UPDATE or
DELETE intents. It returns a completed pass/rejection report, never a writable
candidate or normal-save permit. Inspection/opening remains inert. Native macro
storage discovery, required-macro coverage and absence are not established.

## Working consumer

```sh
tablecraft query prepare-guards 'UPDATE Items SET Score=11 WHERE Id=1' \
  --database example.accdb --guard-property 3 StoredGuard --trace
```

Review exact kind/name keys with `inspect --table Items --extra`. The repeated
`--guard-property KIND NAME` choices explicitly authorize those source-owned XML
properties as guard inputs; property names and namespaces do not detect native
event storage. Missing, repeated or unresolved selections refuse. Caller order
is engineering invocation order, not claimed native dispatch order. Unselected
XML is never parsed as a program or run, and cannot establish required coverage.

The adapter reuses the existing neutral local statement planners and actual
Access snapshot/value-conversion owners. One source is captured, SQL intents are
bound against it, and the guard service reads programs and original records from
that same image. INSERT requires every ordinary field; UPDATE selects one column;
DELETE resolves validated snapshot handles to engine-owned source ordinals.
Defaults, generated values, cascades, external sources and ambient macro functions
are not introduced. Encrypted/deployment source profiles refuse.

Reports name validated selected properties, the decision, optional error data
and optional payload-free visited-node/source-span metadata. Description text is
escaped as data. A completed `Rejected` report is successful command completion;
check **Guard decision** for the guard outcome. Typed preparation failures return
no completed report/stdout. No output pathname is accepted and no copy is created.

## Execution contract

The initial native profile accepts ordinary Int32/NULL fields with known fixed
flags. Existing exact `RowInsert`, `FixedScalarUpdate` and `RowDelete` carriers
remain authoritative. `DeleteOrdinals` is local to the captured image, without
persistent identity or an external raw-record precondition claim. No ordinary
writer, source selector or active-content gate is relaxed.

Every selected property/body/slot is compiled before macro row evaluation,
including unreachable branches. Unknown events, opaque actions/structure,
unsupported references and runtime/external calls fail closed. Statements,
single-If conditionals and RaiseError are the initial action profile. The scalar
parser/evaluator is reused directly inside ACE, without a query-executor or
application dependency. Updated is lowered locally, not registered as a SQL builtin.

| Context | Initial binding |
| --- | --- |
| BeforeChange update | Unqualified current fields, `[Old].[Field]`, `[IsInsert]=False` |
| BeforeChange all-explicit insert | Current supplied fields and `[IsInsert]=True`; Old refuses |
| BeforeDelete | Documented `[Old].[Field]` only; unqualified/current/New/IsInsert/Updated refuse |
| Updated | Literal field selector, update only; provisional actual Int32/NULL value change |

The documented Old binding and RaiseError cancellation are explicit desktop
contracts. `[New]` is not inferred from an internal staged-row name.
[Before Change](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/before-change-macro-event),
[Before Delete](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/before-delete-macro-event).

If must return Boolean. NULL and nonBoolean results refuse instead of silently
adopting WHERE truth or numeric coercion. RaiseError Number accepts a checked
integral signed Long; NULL/Boolean/floating carriers refuse. Description is
optional Text: plain contents are literal text, while a leading `=` selects a
closed expression. Present-empty differs from absent. Expressions have no clock,
random, session, domain, dynamic Eval, provider or UDF callbacks.
[Desktop RaiseError](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/raiseerror-macro-action).

The qualified XML text owner decodes entities/CDATA/comments and XML 1.0 line
endings without changing source coordinates or property bytes. Traces retain
original decoded XML UTF-8 ranges, including BOM; these are not file offsets.
Explicit property selection is bounded at 1,024 entries, compiled instructions at
4,096 and returned trace steps at 65,536. Expression parsing has 4 KiB/128-token/
16-depth limits; literal Description shares the evaluator's 64 KiB text ceiling.
All source, staging, rebind and report work uses cumulative allowances and
cancellation, including the final publication check.

Original caller selectors and typed intents remain separate from staged rows,
compiled caches and produced reports. A final source rebind rereads the original
image/programs and recomputes outcomes/traces using the same qualified pure laws.
It does not trust producer-expanded authority or claim a separately implemented
interpreter. Later-row failure, cancellation or resource failure publishes no
successful prefix. RaiseError yields a completed rejection with no source write,
logging-table write or external effect.

## Observed checks and remaining qualification

The [source prerequisite](../../testing/results/ace16-guard-sources-2026-10-30.json)
qualified 40 original source/intent carriers plus six admitted selector carriers
and two native-Malformed controls before guard execution. Its first selector
source expectation was corrected to respect the existing duplicate-kind MR2
refusal; original program/trace bodies and decoder behavior remained unchanged.
These bytes do not qualify a runtime ambiguity-selection branch.

Native normal compilation, ordinal SOURCE, three core API functions, three
selector functions, four causal authority functions and the example parser pass
on their first executions. The core matrix observes 20 completed reports and
20 typed failures. Authority controls include a valid alternative intent and a
valid selector-B/compiled-B/report-B presented against original selector A.
The first native production lint found one useless error conversion; removing
it changes no behavior or golden.

Host SOURCE passes 40 core and six admitted/two rejected selector carriers. Its
first API target passes three functions: 40 direct Access cases and 40 CLI cases,
each with 20 completed reports and 20 typed/no-report failures, plus 12 CLI
selector-order/isolation calls. Two parser unit functions pass. Broader checks
pass: 540 native mutation functions, 19 Extra functions, 20 preserved-contract
functions (three existing ignored), 63 Access library plus two integration
functions, and 40 CLI unit plus 45 integration functions. Three-package strict
all-target/all-feature checks, minimal ACE/Access builds, formatter, documentation
links and dependency boundaries pass. The strict history retains three native
typed-array style findings and one later CLI loop finding; equivalent corrections
change no behavior or golden. Post-style authority four and example parser one
pass. See the [focused result](../../testing/results/ace16-before-event-guards-2026-10-30.json).

This is the first working execution consumer, not complete desktop macro
semantics. Native dispatch/Updated timing for equal assignments, defaults,
generated fields and cascades remains unqualified. Complete storage detection,
after-events, named calls, further actions/contexts, error handlers/logging,
transaction effects and Microsoft Access save/reopen comparisons remain required
under [data-macro acceptance](../../testing/DATA_MACROS.md). Existing normal ACE
write/save gates remain unchanged.
