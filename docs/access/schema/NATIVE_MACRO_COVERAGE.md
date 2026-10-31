# Source-bound table macro coverage

`NativeImage::inspect_table_macro_coverage(object_id, ...)` retains inert coverage
for one exact local table in an immutable captured image. The report owns the
table's original Lv and LvModule payloads, decoded or opaque LvExtra, and every
observed direct catalog child's identity, native origin and opaque payloads.
Missing columns, stored NULL, present-empty and present opaque bytes remain
distinct. Unknown child kinds remain visible.

Catalog capture supplies the scoped containment mapping. Collection checks that
descriptors and native origins agree, including each child's nonzero ParentId.
Native ParentId 0 remains the no-parent sentinel. Unrelated dangling parents and
indirect or uncatalogued storage remain outside this direct scope. Child bytes
are never classified as programs merely because they resemble XML.

The report borrows the exact original `NativeImage`; its `image()` returns that
same source lifetime. It accepts no replacement source or caller-supplied revision
token. Payload assembly and root Extra decoding use the existing bounded readers;
unsupported storage, malformed framing, allocation, resource or cancellation
failures return no partial report. Collection does not evaluate programs.

## Explicit preparation

With the engineering-preview feature, the report provides
`prepare_before_event_guards` and `prepare_before_change_fields`. Both take exact
property selectors and original intents, without another image or table name.
Before evaluating anything they require:

- The actual captured table name resolves uniquely to the same scoped identity.
- Lv and LvModule are explicitly Absent, with no observed children.
- LvExtra is Present with supported wide-MR2 framing.
- Nonempty explicit selectors cover every observed property, without opaque
  groups, unrepresented names or unresolved XML carriers.

Missing information never satisfies these conditions. Empty row intents still
undergo this preflight. The original standard-capture, Int32/NULL source profile,
selector ceilings and cumulative resource limits remain in force.

The guard method adds complete observed-property selection to the existing
[guard preparation](NATIVE_BEFORE_EVENT_GUARDS.md) profile. It preserves that
compiler's declared-event rules; it does not add dormant-action validation. The
fields method delegates the existing
[complete BeforeChange field preparation](NATIVE_BEFORE_CHANGE_FIELDS.md), which
also validates supported dormant BeforeDelete definitions before rows run.
Unsupported event contexts or actions retain their existing refusals.

Returned logical preparations borrow the original image, so dropping the
report's allocated metadata does not invalidate them. Neither preparation is a
physical edit or a required-event certificate. Existing image-based APIs and
physical write consumers keep their original admission and proof rules.

## Compatibility boundary and evidence

These observations do not establish a complete native macro locator, required
event discovery, macro absence, native dispatch order or normal ACE save/journal
permission. In particular, no observed envelopes is not proof of no macros.

Source-only qualification completed 32 classifications before installation:
30 positive carrier/child views and two intentional malformed-group refusals.
Ten focused functions then passed, covering exact image/origin references,
distinct scopes, all retained payload roles, complete selection, report lifetime,
actual SetField results, legacy event behavior, resource pools and cancellation.
Existing Extra, native guard and guard CLI regressions pass 55, 22 and 20
functions respectively. All-target/all-feature ACE, Access and CLI Clippy,
formatting, and minimal ACE/Access builds also pass locally.
The [result record](../../testing/results/ace16-source-bound-macro-coverage-2026-10-30.json)
keeps compilation failures, corrected source classification and subsequent
verification separate. These authored sources do not qualify native Microsoft
Access execution or save/reopen behavior.
