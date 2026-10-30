# Historical data-macro XML evidence

The [saved verifier](../verify_data_macro_xml.py) replays the original seven
inert inspection calls and their 130 exact records, including nine XML envelopes
and two names-only results. It reads retained originals and captured streams; it
launches no process, imports no witness code and rebuilds no consumer.

```sh
python3 -B docs/testing/verify_data_macro_xml.py
python3 -B docs/testing/ace16_data_macro_xml/test_source_witnesses.py
```

Run without Python optimization, as the existing verifier's proof checks use
assertions. Missing originals, consumer, streams or source witnesses fail rather
than turning into a passing partial replay.

## Exact source witnesses

The [manifest](source-witnesses.json) binds the immutable original result record
and its 13 source pins to retained, byte-identical witnesses. Original source
paths, sizes and hashes in the result, bound protocol, plan and freeze remain
unchanged. The maintained verifier selects a witness only when the requested
original pin exactly matches that binding. It verifies the witness's size/hash
against the original on each use, confines paths to this repository and the
witness namespace, and checks inventory completeness and duplicates. The record,
manifest and witnesses are rechecked before success.

Current production files can evolve without being misrepresented as the source
of the historical executable. There is no fallback from a missing or tampered
witness to current source, even when that current file happens to match. The
captured executable's separate substitution with the identical retained consumer
continues to require its original size and SHA-256.

The historical lib.rs bytes were resolved from the exact matching Git blob at
376aa54faaaedbfa8d4e34d8c7068aa5c90dd906. That establishes the retained file's
identity, not a claim that this commit built the executable. The original result
records a source base and tested source-file pins; it is not upgraded here to a
fresh isolated full-archive build qualification. Witness Rust/Python files are
data only and are never imported, executed or added as build dependencies.

The pure mapping tests use isolated synthetic files. They cover changed current
sources, wrong requested pins, correlated manifest/witness changes, missing or
duplicate mappings, path escapes, record rebinding, mid-replay tampering and the
separate executable substitution. They also retain a Python witness that would
raise immediately if incorrectly imported.

This is maintenance of historical envelope-inspection evidence. It does not
qualify current engine behavior, macro program execution, macro absence, native
ACE16 saves or Microsoft Access timing. New program-validation work has its own
source/API acceptance. The [original result](../results/data-macro-xml-envelopes-2026-10-29.json)
and [data-macro requirements](../DATA_MACROS.md) retain their existing scope.
