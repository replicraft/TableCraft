# Ordinary Byte and Int16 fixed updates

The ordinary fixed UPDATE continuation adds exact native Byte and Int16 targets
through the existing `plan_recoverable_ordinary_fixed_updates` service and retained
Access editor. The new original request constructor is
`NativeFixedIntentGroup::new_ordinary_scalar`; it retains data without admitting a
file. Typed/native requests carry `Value::Byte` or `Value::Int16` exactly. SQL
assignment follows the separately checked integral conversion rules.

Every named target is qualified before no-op filtering, including an empty group.
Fresh complete Source and NoRequired admission remain necessary. Native type,
scalar type, manual fixed flags, width, extended flags, physical span, dependency,
constraint, indexed-target and original-value checks remain. This continuation
changes only unindexed fixed fields; it grants no generated-field, indexed-key,
program or allocation authority.

Byte occupies one native payload byte and Int16 occupies two little-endian bytes.
The independent proof derives those bytes from original requests and checks the
entire final image and fresh final Source. Clearing a nullable cell changes its
presence bit while preserving inactive payload. Expected values compare by their
stored representation. Record geometry, physical ROW identities, other cells,
indexes, counters, maps, catalog, protected bytes and EOF remain unchanged.

## Original request compatibility

Original named target metadata selects the dialect. Naming Byte or Int16 selects
`TCNI0009`/profile 9, including NULL-only, equal-value and named-empty requests.
That selection dominates an IEEE target regardless of group order. Otherwise,
IEEE targets retain TCNI0008 and legacy targets retain TCNI0004.

TCNI9 uses the existing bounded original-group framing, BEFORE binding and
nine-byte cells. Tags 0 through 7 retain legacy/IEEE representations. Byte tag 8
has one payload byte and seven zero padding bytes; Int16 tag 9 has two signed
little-endian bytes and six zero padding bytes. Unknown tags and noncanonical
padding refuse. Old TCNI3/4/8 constructors and decoders remain closed. An old
record naming an empty Byte/Int16 group cannot gain target authority merely
because it contains no carrier tag.

Cold recovery selects the decoded profile once, verifies the original image and
reconstructs its complete native plan against the same checked V1 journal. It
compares both complete images before effects and never retries a failed profile
as TCNI9. BeforeChange, fitted and growth routes retain their existing carrier
policies. A cumulative codec vocabulary does not establish every mixed-schema
write combination as qualified.

## Current evidence

The read-only gate uses two pinned, independently authored exact wide-scalar
images. It checks their originals and four literal in-memory variants: endpoint
batches replace half-open record span `2..5` with `00 ff 7f`; NULL clears change
mask `ff` to `f9` and retain inactive bytes `ff 00 80`. Six complete read/raw
contexts, six NoRequired admissions and six canonical Source admissions passed.
No new database files or matching-writer expectations were constructed.

Six native commits, six typed session saves and six CLI save/readbacks match the
literal endpoint/NULL/restore images. Direct reconstruction rejects sixty
protected-byte forgeries and six wrong counts. Exact/no-op/empty, correctly typed
and inactive-NULL stale witnesses, old4/8 cold refusal, six public resource
operations, twenty recovery contexts and sixteen mixed-kind/order contexts pass.
The endpoint CLI uses plain integer literals through the shared checked assignment
service; restores retain explicit CByte/CInt constructors. Seventeen codec and
three literal-wire functions pass, along with 109 unique retained runtime
functions and strict/minimal/dependency/format checks. See the
[execution record](../../testing/results/ace16-normal-byte-int16-update-2026-11-03.json). The new variants have no independent SDK or native Microsoft
Access interpretation claim. Windows save/reopen, mixed-client and power-loss
qualification remain separate and unrun.
