# Shared native append state

The feature-gated ordinary-variable and index-split previews now use one shared
append-state implementation. Each actual standalone operation owns one original
length, current EOF, ordered typed reservation list and original-before/current-after
map-byte ledger. DATA, physical index and index-map bitmap reservations advance
that same bounded cursor. There is no second mutable length or global-byte vector
in either owning plan. Original map references and page-membership inventories
remain role-specific facts.

This is a working extraction for the existing [variable](ACE16_VARIABLE_UPDATES.md)
and [encoded/numeric split](ACE16_ENCODED_INDEX_SPLITS.md) consumers. It adds no
combined DATA-plus-index update API or normal write path. A private mixed-purpose
cursor test checks the mechanical sequence; it is not a qualified combined image.

Ordinary owned/candidate/global cells retain their existing unique-claim behavior,
including a legitimate provisional new-DATA hint set followed by clear. Index
owned/global byte merging retains its existing behavior. Different roles editing
one byte compose against one original value, with inconsistent before bytes
rejected. Cell grammar, polarity, membership and actual failed-fit authority still
belong to their qualified callers and independent audits; the shared byte helper
does not grant writable-cell authority.

Existing ordinary highest-hint selection, record placement and represented-only
allocation are unchanged. First forwarding retains the original root's physical
range, clears that ordinary body then writes its pointer; already-forwarded root
remainders and retired hidden bodies retain their existing preservation policy.
Existing index page order, repeated split policy and represented/promotion
behavior are unchanged. Index reservations and separate bitmap reservations
retain exact physical owner identities. Pair cancellation cannot leave a partial
cursor reservation.

The existing index audit still derives its own cursor, map intents and topology;
the DATA audit remains separate. Builder reservation labels are additional
consistency facts, not independent allocation authority. In particular the current
standalone variable map replay still derives event lengths from its staged plan.
A future combined service must first derive DATA lengths/placement from immutable
requests and original record facts, then independently compose DATA and index
effects. This checkpoint does not claim that future authority is implemented.

Existing admission remains intact: the complete shared census remains required
for existing index promotion, and is not made unconditional for old represented-only
index or variable operations. Stronger combined-operation census/purpose gates
remain future work. Unknown metadata and missing/contradictory representations
keep their existing domain refusals; no new grammar or in-file reuse is added.

The [scoped result](results/ace16-shared-append-state-native-2026-10-27.json)
records a pre-correction batch of 123 distinct runtime functions, including three
new private functions, and three compile-fail doctests. The selected logs contain 130 executions, with
seven repeated shared parser/publication executions excluded. That batch passed existing complete numeric, encoded and variable images, actual
consumers and private preservation controls, plus feature/default strict, minimal
and scoped formatting checks.

A later private byte-ledger discriminator exposed an inconsistent-original-byte
check missing from the provisional hint-clear fast path: the healthy set-to-clear
sequence passed, but a forged before byte also returned success. The helper now
rejects that mismatch before mutation and pays for its cancellable search. This
is a private primitive invariant correction, not observed public-input image
corruption. The first failing run and frozen pre-correction sources remain
unchanged. After the correction, the causal control and 17 distinct append/DATA
functions pass, as do feature strict, minimal and two-file formatting checks.
The full 123-function batch was not rerun after this narrow change. New typed reservation storage and checks add paid work;
identical resource/cancellation boundaries or performance improvement are not
claimed. No original-file candidate, JVM or public writer process ran in this
implementation checkpoint, and no native Access save qualification follows.
