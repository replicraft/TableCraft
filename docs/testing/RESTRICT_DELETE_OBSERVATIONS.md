# Cross-table RESTRICT DELETE observations

The [candidate receipt](results/ace16-delete-restrict-original-replay-2026-10-26.json)
records **84 controls, zero final-image-model discrepancies and six public-action
disagreements**. The frozen candidate produces 64 successful images and twenty
typed `MissingParent` refusals without output. Public Jackcess per-row actions
instead produce 58 successes and 26 byte-identical constraint refusals. This is
**not full public action parity** or native Microsoft Access SQL/action/save
qualification. Both metrics and all original selectors remain visible.

## Two new original identities

Historical registered FK/update/cascade inputs were unavailable in the inspected
local scope: a bounded hash lookup of 740 existing repository ACE16 databases
found none of fourteen relevant registered hashes, and the historical `/tmp`
filenames were absent. That lookup was recorded as tool output, not a retained
raw scan log. No historical bytes were regenerated or described as restored.

The approved replacement comparison uses two **new** V2016/V2019 files, with fresh
hashes in [the fixture ledger](FIXTURES.md) and
[reference receipt](results/ace16-delete-restrict-original-reference-2026-10-26.json).
[RestrictDeleteFixture.java](RestrictDeleteFixture.java) is original repository
code under the root license. It uses reviewed Jackcess 5.0.4, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
No upstream implementation or fixture is copied. New paths are required;
accepted originals and receipts are never overwritten.

Each original has nineteen tables and 133 rows: seven scalar parent/child pairs,
two fixed composite pairs, and a second I32 child table. Parents use a separate
non-NULL ID primary index and an ordinary unique nullable relationship-key index.
The seven key families are Byte, Int16, Int32, Int64, Currency, GUID and exact
precision-28/scale-4 Decimal; composites are I32/I64 and Decimal/I32. Values include
bounds, NULLs, duplicates and adjacent exact wide values beyond f64 precision.
Java getter spelling `Byte:-1` represents stored unsigned Byte 255; the independent
model normalizes that byte explicitly rather than treating it as a signed key.

Actual headers are plain, subversion-zero ACE16. BigInt uses native code 19,
width 8; Decimal uses code 16, precision 28, scale 4 and width 17. Getters identify
ten enforced cross-table relationships, all delete-RESTRICT: five use update
RESTRICT and five update CASCADE. There are no self links, delete cascades or SET
NULL actions. Measured selected-table properties, catalog-owned child objects and active
payloads remain empty/NULL. No flags or headers were patched to force eligibility.

The separate peer [metadata audit](results/ace16-delete-restrict-original-reference-2026-10-26.json)
pins its ignored artifact and scope. It checks nineteen normal catalog entries,
48 root leaves/profile and twenty reciprocal endpoints. It is read-only metadata
evidence, not another engine, reference-action or candidate execution. Its nine
reviewed engine fingerprints match historical revision `1e776246`, not later
working-tree files.

## Actions, histories and the preserved discrepancy

There are 42 selectors/profile, 84 total:

- For all seven scalar families, delete a non-NULL child, an unreferenced parent
  key 30, and a referenced parent key 10.
- For Int64/Currency/Decimal, delete an unreferenced key adjacent to a referenced
  exact wide key. I32 additionally covers NULL child and NULL parent deletion.
- Each composite pair covers all-NULL child deletion, an unreferenced whole tuple
  whose individual components appear elsewhere, referenced partial-NULL parent
  refusal, removal of both all-NULL parents, and a child-then-partial-parent history.
- The I32 two-child-table history removes matching roots from the first child,
  verifies the second child still blocks the parent, removes that second child,
  then removes the now-unreferenced parent.

All write copies explicitly enable foreign-key enforcement, disable expressions,
use UTC and reject linked opens. Actions call public `Table.deleteRow` individually;
a successful multi-row request records those calls without asserting SQL statement
semantics. Each refused action is checked after closing the writer: its complete
copy bytes equal its pre-action source. Getters reopen read-only. Originals and
all reference states remain unchanged during candidate replay.

The independent existing final-image rule exempts an all-NULL child tuple; a
partial-NULL child still requires an exactly matching complete parent tuple.
Six public actions disagree with the resulting deletion model: I32 NULL parent
ID 6 and both composite all-NULL parent IDs 6/7, in each profile. The public writer
refuses those requests unchanged; the candidates remove exactly the selected
parents while all final relationships remain valid under the explicit model.
The candidates are not compared to the unchanged rejected-reference images for
these six cases. Expected survivors, full retained encoded keys and final membership
are modeled independently. All other 78 public action outcomes agree.

[RestrictNullJoinProbe.java](RestrictNullJoinProbe.java) separately uses read-only
public `Joiner` lookups. The NULL scalar parent matches NULL child IDs 4 and 2;
each all-NULL composite parent matches child ID 3. Both original hashes remain
unchanged. This records action-time NULL-key lookup behavior; it does not turn
that lookup into a native SQL timing rule or alter the final-image contract.

The initial reference run intentionally stopped after 26 controls at its first
discrepancy and exited 1. Its exact source/report/log are preserved and its coverage
is not added to the complete 84-case run. The complete reference run also exits 1,
retaining all six disagreements. No selector or original expected outcome was
removed or relabeled as model agreement.

## Frozen artifact and independent image proof

The tested example is `.cache/tablecraft-bin/ace16-delete-restrict-20261026`,
SHA-256 `aeb9a06d16d7d6356fe7ab0f2c53afd9f96f851da587ec0b407b5aff3c088f08`,
compiled at `1e776246313e114c7ea0d8f4b18fe3fc4162ef7f`. Its
[build receipt](results/ace16-delete-restrict-example-build-2026-10-26.json)
records clean compiler inputs. All 23 compiler object hashes were independently
checked with `git show` at that revision. Later working-tree changes are not
claimed to match the frozen artifact.

For all 64 positive images the verifier checks exact selected-root deletion,
complete final relationship membership, retained encoded keys/references, source
leaf partitions, full key-and-row separator maxima, original nonempty heights,
fixed file length, exact map bits, original retired residuals and every other
byte. Historical unique-entry counters remain unchanged; they are not live counts.
Twenty expected failures return typed `MissingParent`, empty stdout and no output
file. Sixteen history steps consume prior successful **candidate** images rather
than sequential-reference bytes; rejected steps retain the previous source.

This remains a separate non-journalable engineering candidate. Unsupported actions,
self relations, non-enforced/unknown metadata, unsupported key profiles, normal
writes and unqualified active-content/data-macro behavior are not enabled.

## Reproduction and version provenance

[Public reference tooling](restrict_delete_originals/reference.py) has explicit
path/header guards. Its exported model and plan functions retain the accepted
Python AST and regenerate all 168 actual reference/getter argv unchanged. New
main-entry guards were checked read-only; they were not present in the historical
capture. Exact [complete](restrict_delete_originals/historical/reference-complete.py.txt)
and [initial-stop](restrict_delete_originals/historical/reference-initial-stop.py.txt)
sources remain archived. The public
[candidate](restrict_delete_originals/candidates.py),
[getter](restrict_delete_originals/getters.py) and
[support](restrict_delete_originals/support.py) modules and both Java helpers are
byte-identical promotions of their executed private sources. Shared atomic and
raw framing helpers are reused read-only. Expanded outputs and databases remain
ignored under `.cache/restrict-delete-originals-next-20261026/` and hash-pinned;
public labels are repository-relative. Do not use Python `-O`.

Compile both Java helpers into a fresh repository-local class directory. Use the
platform's classpath separator; the Python helpers use `os.pathsep`. Create new
original paths, never the accepted ones:

```sh
javac -cp "$jar" -d "$classes" docs/testing/RestrictDeleteFixture.java \
  docs/testing/RestrictNullJoinProbe.java
java -cp "$classpath" com.healthmarketscience.jackcess.impl.RestrictDeleteFixture \
  generate "$fresh2016" V2016
java -cp "$classpath" com.healthmarketscience.jackcess.impl.RestrictDeleteFixture \
  generate "$fresh2019" V2019
```

Set `classpath` to the reviewed JAR plus that class directory. Capture a read-only
`oracle PATH TABLES` for each file; `TABLES` is the comma-joined
`reference.tables_list()`. Name the captures `V2016-before-getters.tsv` and
`V2019-before-getters.tsv` in a fresh baseline directory. Then run:

```sh
python3 docs/testing/restrict_delete_originals/reference.py \
  --input V2016 "$fresh2016" --input V2019 "$fresh2019" \
  --baseline-dir "$baseline" --helper-dir "$classes" \
  --helper-source docs/testing/RestrictDeleteFixture.java --jar "$jar" \
  --workdir "$fresh_references" --report "$reference_report"
python3 docs/testing/restrict_delete_originals/candidates.py \
  --references "$reference_report" --baseline-dir "$baseline" \
  --helper-dir "$classes" --helper-source docs/testing/RestrictDeleteFixture.java \
  --jar "$jar" --example "$frozen_example" \
  --example-sha256 aeb9a06d16d7d6356fe7ab0f2c53afd9f96f851da587ec0b407b5aff3c088f08 \
  --compiled-revision 1e776246313e114c7ea0d8f4b18fe3fc4162ef7f \
  --workdir "$fresh_candidates" --report "$candidate_report"
```

The reference command's six-discrepancy exit 1 is intentional evidence, not a
reason to erase the report. The candidate command exits 0 only when all 84 final
model expectations hold; the separate public-action metric still reports six.
Run from the repository root with fresh outputs inside the checkout. New generation
bytes may differ and require their own receipts. This does not recreate missing
historical originals or establish Microsoft Access action compatibility.
