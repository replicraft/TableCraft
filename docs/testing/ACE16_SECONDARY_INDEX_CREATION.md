# Physical secondary-index creation

Status: native engineering creation API, exact quota/purpose controls, mutation
regressions and strict/minimal builds pass. The Access adapter and explicit
[SQL/CLI copy command](../access/writes/SQL_ENGINEERING_COPIES.md#explicit-secondary-index-creation)
now consume this service. Native Access creation/save qualification remains open.

The [source gate](../../crates/engines/tablecraft-ace/src/table/mutation/create_index_tests.rs)
constructs eight authored BEFORE images and sixteen complete ASC/DESC expected
images across headers 5 and 6. Its first execution passes one function covering
16 pairs and 32 complete reader/ownership views, before any new writer/API.
These are literal engineering fixtures, not Access-created databases or native
Access creation/save qualification.

Each source has Id primary/index aliases, nullable unindexed Score, Guard
secondary/index aliases, and a separate Sentinel table. The expected image adds
a genuinely new ordinary nonunique physical Score index, including NULL entries;
it does not reuse an existing access path. No new primary, foreign, UNIQUE,
Required or IgnoreNULL policy is qualified.

| Population | Rows | Fresh distinct-key counter |
| --- | ---: | ---: |
| Empty | 0 | 0 |
| Scores -1, 0, 1 | 3 | 3 |
| NULL, -7, -7, 0, NULL, 7 | 6 | 4 |
| Fixed mixed NULL/nonNULL pressure | 600 | 541 |

The pinned independent Jackcess writer initializes the new count descriptor to
zero and populates every original logical ROW. Ordinary flags 0x80 leave
IgnoreNULL unset. Duplicate key families retain distinct ROW entries without
incrementing that family counter; NULL contributes one family when present.
These initialization facts come from reviewed writer code, not the reader's
acceptance of arbitrary historical counters. Existing counters 1000/2000 remain
unchanged. This reference profile is not Microsoft Access conformance evidence.

The gate requires every old/new logical index to be Checked, compares complete
KEY/ROW streams, validates physical identities and map references, and runs the
actual source-wide DATA/INDEX/map ownership census. Old rows, aliases, histories,
Sentinel and every unrelated byte remain intact. It separately checks relocated
definition sections, exact GLOBAL bit changes and the inactive definition suffix.
The expected new container/root are pages 24/25, with pressure leaves 26/27;
the single-page definition grows from 500 to 622/624 bytes. These are explicit
literal model choices, not claims about native allocation order.

```sh
cargo test -p tablecraft-ace --all-features --lib --locked \
  qualify_real_secondary_index_source_and_complete_expected_before_writer
```

The [source result](results/ace16-secondary-index-creation-sources-2026-10-30.json)
retains the original model, source gate, helpers, review and first log. Its
SOURCE-only state is historical; it is not retargeted to the later producer.

## Native engineering API

`NativeImage::preview_ace16_create_secondary_index` consumes one Standard capture
and an original borrowed `SecondaryIndexCreate::new(name, column, descending)`.
It returns `Ace16IndexCreatePreview`: complete original/candidate bytes and the
created name, separate logical/physical numbers and indexed-row count. Indexing
records does not report changed data rows. There is no normal-plan, journal or
save conversion.

The initial profile requires a qualified ordinary fixed Int32 table with
contiguous native fields, existing single-column indexes, a supported one-page
TD tail and representable inline allocation records. The selected field must be
previously unindexed. New names are ASCII, at most 64 bytes, beginning with a
letter and continuing with letters, digits or underscore. The new index is
ordinary nonunique/include-NULL, ASC or DESC. These are current engineering
limits, not the complete ACE 16 compatibility target.

Source qualification, all original rows, fresh-key population, definition
capacity and complete source ownership are checked before reservations. A
source-bound fresh owner allocates a distinct map container and INDEX bodies
under explicit new-purpose ledger entries, without inventing an original
`NativeIndex`. The existing key codecs and actual shared page constructor build
one leaf, or a root and two fitting leaves; larger heights, chained definitions,
map promotion and wider field/policy profiles remain refused.

The independent audit rereads original records and the original caller request.
It independently reconstructs NULL/nonNULL keys, distinct-family history, TD
framing, maps and every appended page, checking the plan and ledger as
observations. Complete expected bytes must equal the candidate before final
reader/constraint/ownership checks and cancellation. Old rows, aliases, map
records, counters and unrelated inactive bytes are protected.

First native compilation, the unchanged SOURCE gate, all three initial public
API functions and all three correlated audit controls pass. The API covers all
16 complete expected images; authority controls reject a valid alternative
DESC population against original ASC intent, correlated wrong fresh history,
and correlated omitted GLOBAL claims. A static install review corrected one
explicit LimitError-to-InspectError mapping before any compilation; no source
or expected image changed.

The pressure-source controls pass exact and one-below Bytes/Rows/Steps limits,
including all exact pools together and cancellation at the final observed poll.
Both headers reject FixedUpdate-purpose captures before operation spending or
polling, for empty and 600-row populations. The full mutation regression passes
549 functions; the explicit private-export helper is ignored. Minimal ACE passes.
The first strict check found only a range-expression style lint; an equivalent
inclusive-range correction preserves both endpoints. After that correction, all
five public API functions, all three audit controls and native strict
all-target/all-feature checks pass. Final formatter, documentation links and
dependency boundaries pass. Source/expected images remain unchanged.
The [native result](results/ace16-secondary-index-creation-native-2026-10-30.json)
pins the initial and final logs separately, including the first strict refusal.

Blank-database creation, general CREATE/ALTER TABLE, required macro coverage,
transactions and normal ACE saves remain separate unfinished work. A planned
explicit SQL `query preview-create-index` consumer will lower the existing
CREATE INDEX syntax into the same typed request; it is not implemented yet.

## Independent public-API reference

The first Jackcess comparison stopped on its first read-only open, before any
index creation. The authored system catalog has a zero free-space-map reference;
Jackcess requires that map during bootstrap. All source/expected images stayed
unchanged, and the remaining 55 planned calls were unrun. The frozen corpus was
not patched to make this independent reader accept it.

A separate corpus was then generated through Jackcess 5.0.4's public API. All
48 first-run processes pass: eight source creations, eight read-only source
getters, sixteen actual index creations and sixteen fresh read-only getters.
Both profiles, all four populations and both directions retain complete rows
and the frozen semantic index order. New nonunique/include-NULL indexes have
flags 0x80 and distinct-family counters 0/3/4/541, including one NULL family and
all duplicate ROW entries.

Generated catalogs, existing counters and physical allocation differ from the
authored byte models. These are semantic reference results, not whole-image
equivalence or direct qualification of those authored files. All generated
sources and the original authored images remain unchanged. Reference databases
use the pinned library's embedded templates privately; no third-party fixture
bytes are added to the repository. The [reference record](results/ace16-secondary-index-public-reference-2026-10-30.json)
retains the initial refusal, unrun calls, frozen programs and separate passing
series. No Microsoft Access, native-save or transaction qualification is implied.

## Native admission of generated originals

A separate ignored SOURCE diagnostic now passes on all eight immutable originals
created by the pinned Jackcess public API. Both headers and the empty, three-row,
six-row nullable/duplicate and 600-row populations are included. All sixteen
original ASC/DESC requests pass the actual creation source qualifier. All eight
original table checks and independent existing-owner censuses pass, and all eight
original hashes remain unchanged. No writer, reservation or candidate is invoked.

The exact file hashes and requests were frozen before the first call. Static
review corrected two budget-error conversions before compilation; the original
private draft and freeze remain retained. The diagnostic and native strict
all-target/all-feature check pass on their first executions. The
[SOURCE-admission record](results/ace16-secondary-index-genuine-source-admission-2026-10-30.json)
pins the inputs, installed diagnostic, relevant owners and actual logs separately
from the historical authored-corpus/native-writer records.

This establishes existing-source admission only. It does not yet establish
prospective allocation admission, canonical whole-image writer output, candidate
reopen or Microsoft Access save behavior on these generated originals. A later
whole-image comparison must first freeze independent expectations from these
originals and the original requests; public reference AFTER bytes cannot supply
those canonical expectations.

## Independent complete images for generated originals

A private pure constructor now qualifies all eight original files before deriving
sixteen complete canonical images from those originals and the frozen ASC/DESC
requests. No native writer, Java process, reference AFTER image or candidate
participates. All original hashes remain unchanged. Both previously completed
empty-image hashes remain exact after the immutable-buffer correction.

The model projects the actual original catalog identity, native-numbered fields
and original logical ROWs, verifies complete old key streams and maps, then
independently emits the requested TD insertion, new count/map and fresh INDEX
pages. It protects old rows, existing counters/aliases/index pages, inactive TD
suffix and every unrelated original byte. Canonical append placement is an
engineering policy; it does not claim Jackcess or Microsoft Access packing parity.

Two pure authoring failures remain preserved. The first stopped before admitting
source one because the reused decoder required a present first bitmap pointer;
the legitimate empty owned map has seventeen absent references. A new private
wrapper accepts only an exact checked 69-byte kind1/all-zero-reference row,
leaving the maintained helper unchanged. The second admitted all eight sources
and emitted two empty images before a mutable-buffer decoder TypeError. Freezing
the fully assembled image as immutable bytes corrected that boundary without
changing construction choices. The final pure run passes all sixteen images.

This expected-model checkpoint preceded native writer comparison and fresh
independent-reader candidate opens. The later phases below record those outcomes;
no normal-save or transaction permission follows from this model.

The [expected-model record](results/ace16-secondary-index-genuine-expected-2026-10-30.json)
pins the source-only derivations, retained failures and sixteen complete hashes:
7,208,960 bytes and 2,436 indexed ROW observations across the sixteen cases.
At that expected-model checkpoint, the separately registered ignored native
consumer was still unexecuted; the following phase records its later execution. Its
engineering-test-only `TABLECRAFT_FIXTURE_ROOT` selects the immutable input
directory independently of the committed producer source/build directory; all
frozen inputs are checked before any later producer call.

## Native complete-image comparison on generated originals

The first fresh committed-source producer run now passes all sixteen native
creation calls against the independently frozen complete expected images:
7,208,960 bytes across both headers, four populations and both directions. All
source/expected preflight checks finish before writer one. Every call captures
the original independently; no candidate becomes a later source. API metadata,
all five native indexes, fresh historical counter and complete typed new-index
ROW order pass readback. All eight original hashes and expected inputs remain
unchanged.

The producer was compiled from committed c3a61ddd in an initially absent target,
using the archived 5,285-file source inventory and the explicitly recorded debug=0
configuration. Cargo reports fresh=false for the rebuilt test artifact. The
engineering fixture-root setting keeps private inputs outside that source
archive; it does not alter capture, index or normal-save admission.

The initial no-run compilation stopped on two hybrid-array digest display
errors. A per-byte standard-library formatter fixes the test display boundary;
corrected no-run compilation and native strict library/all-test/all-feature checks
pass. No source or expected byte changed. Both earlier pure authoring failures
remain recorded separately.

These matches qualify canonical engineering output on these originals. They do
not claim Jackcess packing parity, Microsoft Access native saves, macro permission
or a journalable normal plan.

The [genuine-original native record](results/ace16-secondary-index-genuine-native-2026-10-30.json)
pins the actual producer archive, source inventory, executable, build receipts,
native log, sixteen actual candidates and the independent expected-output freeze.
The earlier SOURCE and pure-model records remain historical and unchanged.

## Fresh independent candidate readers

A separately frozen phase passes sixteen fresh Jackcess read-only JVMs on the
actual native candidates. All sixteen whole-image matches finish before binding
reader one; complete comparisons of all candidates against their prefrozen
images repeat before and after every read. All eight originals and sixteen
candidates remain unchanged. No candidate writes occur.

The existing bounded reader disables expression evaluation and denies external
links. Its exact stdout comparison checks original rows, all five index
names/numbers/flags/policies/counters, complete selected typed ROW order and the
untouched Sentinel. Counters are 0/3/4/541. All sixteen stderr streams are empty.
Across the cases, both ordinary and selected-order reads cover 2,436 rows each.

Static review corrected the binding parser before its first execution: the exact
known Rust test-reporter prefix is allowed only on the first MATCH line. The
original unexecuted parser, native raw log and receipt remain preserved; the
producer was not rerun. The executed SDK plan directly pins its reader inputs
and producer manifest. Separate post-native producer-asset rehashes are recorded
as documentation checks, not retroactively claimed as SDK pre-run checks.

The same [genuine-original record](results/ace16-secondary-index-genuine-native-2026-10-30.json)
pins the actual bound plan, execution freeze, reader/comparator, report, command
receipts and all sixteen stdout/stderr pairs. This is Jackcess fresh-reader
evidence, not Microsoft Access native-save or transaction qualification.
