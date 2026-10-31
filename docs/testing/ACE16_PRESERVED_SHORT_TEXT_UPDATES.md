# Fitting Short Text copies in mixed schemas

`NativeImage::preview_ace16_preserved_text_updates` admits one ordinary,
nonindexed Short Text field outside calculated dependencies in a qualified mixed
schema. It takes existing `SlotScalarUpdate` requests with snapshot-local row
ordinals and exact Text/NULL expected values. The result remains a separate
engineering candidate, without normal-save, journal or macro permission.

## Record and value rules

Each changed record keeps its original absolute position and physical length.
The selected payload, its presence bit and variable offsets may change. Other
variable spans may move inside that record, but their complete original bytes
remain intact. Fixed fields, neighboring mask bits, newly unused slack and every
byte outside the selected records remain original. No DATA, INDEX or LVAL page
is allocated or retired, and no forwarding or index maintenance is attempted.

Text uses the existing declared-compression encoder within actual record
capacity. Raw UTF16 remains available when only that smaller representation fits.
Present empty Text and NULL remain distinct. Equal Text preserves its original
encoding; NULL-to-NULL preserves inactive payload bytes and offsets exactly.
An oversized value returns a typed refusal without trying another placement.

The existing preserved-source owner certifies complete records, properties,
calculated dependencies, attachment associations, LVAL ownership, indexes and
allocation roles. Its relationship, property and companion-field limitations
still apply. The metadata predicate `is_ordinary_short_text` alone grants no
write eligibility. Boolean, Int32 and ordinary fitted writers retain their
existing implementations and resource behavior.

## Independent authority

The producer reuses the existing declared encoder and supplied-payload record
composer. A separate proof recaptures the original source and reconstructs each
changed record from original requests through the independent record serializer.
It decodes the selected result, verifies the distinct changed-row count and
compares the complete file, including EOF. Staged payloads, projections and row
lists do not authorize the proof.

The proof also reads the complete original scalar projection from its own
expected image and checks it against freshly read original properties. Required,
AllowZeroLength and qualified field/table rules therefore apply to independently
reconstructed values. The final observed candidate then passes preserved-source
capture again before it can return. Empty and no-op requests retain these gates.

## Shared typed and SQL consumers

The projected `AccessEngineeringEditor` and shared `FittedUpdate` service select
this target under Default mode. The typed command keeps each caller-supplied
Text/NULL expectation and row ordinal; it does not replace expectations with a
new snapshot. Actual native metadata selects the preserved fitting service.
Existing `query preview-update` and typed `write preview-update` commands need no
new flag or public request type:

```sh
tablecraft query preview-update "UPDATE Items SET Short_Text='beta' WHERE ID=1" \
  --database source.accdb --output edited.accdb

tablecraft write preview-update source.accdb edited.accdb \
  --set Items Short_Text 1 text null é
```

Examples require an admitted native schema and a fresh output path. The typed
ordinal is snapshot-local; the second example requires row 1 to contain NULL.
Explicit variable, growth, LVAL and larger-image modes retain their existing
profiles. Multiple targets and fixed/variable mixtures remain refused for this
preserved Text path. No failed mode is retried through another writer. Ordinary
tables keep their existing routing, and the native engine still owns final
metadata, dependency, ownership, capacity and constraint admission.

The initial full frontend targets passed 17 Access and 16 CLI functions, with
12 principal complete-image comparisons. The added shared `FittedUpdate`
acceptance passed all eight positive images. Its first direct typed CLI test
used database/output flags where the existing command requires positional
SOURCE/OUTPUT; that run retained 18 Access passes and 16 CLI passes plus this
one invocation-only failure. Correcting only the test invocation yields 18
Access and 17 CLI passes, with 28 principal complete-image comparisons:
12 earlier comparisons, eight shared fitted copies and eight direct typed CLI
copies. Stale/foreign-handle, overflow, mode, source-profile, resource and
no-clobber controls remain in place. Strict checks for ACE/Access/CLI across all
targets and features, ACE/Access no-default-feature builds and workspace formatting
pass. The final independent proof rerun passes four functions. Existing typed
regressions pass eight preview and five file-input functions, including four
ordinary Default Text whole-image copies through argv and TSV literal-null text
inputs. Documentation validation passes 6,742 targets with no missing links, and
the diff check passes. The final native contract rerun passes 33 functions, with
two existing helpers ignored.

## Authored-source evidence

The unchanged rich source already contains Short Text, stored calculations,
attachments, Memo/OLE, Hyperlink and extended dates. Its original Short Text has
raw UTF16 `alpha`, a NULL with inactive `D1 E2` bytes, and `omega`. Literal
expectations cover shorter narrow and Unicode text, exact-capacity Unicode,
NULL-to-value with raw fallback, value-to-NULL, empty Text and both no-op forms.
Overflow has no expected candidate.

Selected readonly views pass before producer installation. Complete preservation
capture then admits 17 header-6 original/expected views and retains 17 header-5
refusals for the corpus's Extended Date declarations. This is not a general
header-5 write refusal. A separate healthy original with AllowZeroLength disabled
passes source admission before its empty-text request is tested.

Eight complete native images match. The native contract passes 33 functions,
with two existing helpers ignored. Four independent proof functions reject
unrelated page/cache/mask changes, changed inactive NULL bytes, wrong counts,
alternative requests and independently reconstructed constraint violations.
The [result record](results/ace16-preserved-short-text-2026-10-30.json) retains
the static physical-count framing correction and first fixed-span transport
failure, first fixture lint and additional typed CLI invocation failure separately
from subsequent passing checks. The fixture lint correction replaces a constant
`chunks_exact(2)` call with equivalent typed-array `as_chunks` handling; literal
source and expected bytes remain unchanged. Source and literal expected
bytes were not retuned from producer output.

These are authored-source engineering checks. Genuine-file qualification,
Microsoft Access save/reopen, required-macro discovery and ordinary ACE saves
remain separate work.
