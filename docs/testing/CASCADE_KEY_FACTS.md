# Projected cascade association facts

The engineering cascade service now retains only columns used by touching
Cascade relationships for association matching. It maps projected row positions
back to native declarations for primary-key order and Decimal parameters. Empty
projections still retain every original row/root association. Complete preflight,
field/rule/relationship validation and final-image audits are unchanged.

This removes one redundant Memo/OLE decoding pass and its retained values. It
does not eliminate all long-value reads, increase decoder limits, qualify new
write profiles or enable ordinary saves. Matching and validation remain bounded
by the same operation allowance.

## Measured native workload

[CascadeKeyFactsFixture.java](CascadeKeyFactsFixture.java) creates two fresh
ACE16-family originals through the reviewed Jackcess 5.0.4 public builders, with
expressions disabled, UTC explicit and linked opens refused. Each file contains
three parent rows and six child rows. Composite relationship order differs from
native column order; unindexed Memo/OLE columns occur before and between keys.
Populated payloads contain 65,536 UTF-16 Memo units and 131,072 OLE bytes.
NULL and empty payloads are distinct. The nine rows use 396 actual LVAL pages.

The frozen preceding tool and the new implementation produce byte-identical
candidates for parent, all-parent and child-only requests in both profiles.
Independent public getters verify survivor identities, relationship components,
payload lengths/hashes and original physical row locations. The
[scoped receipt](results/ace16-cascade-key-facts-2026-10-26.json) pins the inputs,
source, raw measurements, initial diagnostics and completed checks.

For V2016, whole-operation cumulative charges change as follows:

| Request | Charged bytes before | Charged bytes after | Work steps before | Work steps after |
| --- | ---: | ---: | ---: | ---: |
| First parent | 25,286,793 | 21,697,470 | 21,636,717 | 18,482,278 |
| All parents | 19,879,713 | 16,290,390 | 16,903,405 | 13,748,966 |
| Child only | 17,031,845 | 14,638,855 | 15,753,348 | 13,650,391 |

The retained facts pass reads zero cold LVAL pages, versus 132 parent and 264
child page reads in a full snapshot. These are reads from an already captured
in-memory image. Charged bytes measure cumulative accounting, not peak memory;
work steps are not elapsed time. The receipt retains separate V2019 counters.
No disk-I/O, throughput or wall-clock speedup is claimed.

## Validation and reproduction sources

Four focused engine tests cover dense/native position mapping, reversed composite
order, zero-column facts and complete unselected-field validation. The scoped
regression batch passes 208 engine tests with one existing ignored test; strict
lint, minimal compilation and formatting pass. An additional explicit corpus
probe passes two tests, separately from those engine counts.

[probe_cascade_key_facts.rs](probe_cascade_key_facts.rs) is the exact executed
measurement/corruption probe, kept outside normal Cargo test discovery. It needs
an explicitly prepared corpus selected with `TABLECRAFT_FACTS_CORPUS`, including
raw LVAL-page inventories, original requests and frozen-before candidates. It is
not a default test or a claim that ignored corpus files ship with the repository.
The receipt identifies the original preparation/getter captures and sources.

The corruption control changes only private copies of LVAL owner signatures.
Key projections still succeed, while both complete table reads and cascade
preflight return `Malformed`. Original inputs remain unchanged. This demonstrates
that projection does not bypass complete payload validation.

Initial preparation rejected an incorrect header-byte assumption. The first
metrics invocation also used a relative corpus path under Cargo's package working
directory and failed before opening a database; the same archived probe then
passed with an absolute path. Both diagnostics are retained. Neither required a
production correction or changed candidate expectations. Native Microsoft Access
SQL/action/save qualification remains separate.
