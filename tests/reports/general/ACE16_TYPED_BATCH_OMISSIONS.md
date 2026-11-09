# Typed batch INSERT omissions

`tablecraft write preview-insert SOURCE OUTPUT TABLE rows.tsv` now accepts
`omitted` KIND with empty DATA. Each row still supplies KIND/DATA pairs in native
column order. `null` KIND with empty DATA remains supplied NULL; Text DATA
`omitted` or `null` remains literal text. Missing pairs and inconsistent row
widths refuse rather than inventing omitted fields.

A batch containing omissions retains the original supplied/omitted intent for
every row through one existing native INSERT preview and independent audit.
Only ACE resolves needed qualified defaults or nullable NULL fallback. Complete
batches keep the existing explicit-row API, passive default metadata and exact
carriers. No SQL grammar or defaults evaluator was added.

`--incrementing` selects the existing engineering increment-by-one policy using
LAST from this same captured image. `--incrementing-last N` adds the caller's
exact LAST precondition. TSV then excludes only the actual generated primary
column; neither NULL nor omission infers generation. Stored Increment/Random
mode is not detected. Generated keys are returned only after the complete
candidate passes native checks.

Explicit `--default-clock[=LOCAL-DATETIME]` authorizes the existing lazy provider
for needed stored Date()/Now() defaults on legacy DATE fields. All needed
bindings and generation bounds precede its one operation reading. Supplied
values, supplied NULL and batches needing no clock do not read it. This retains
the existing engineering timing policy, without native Access timing claims.
The actual provider moved byte-for-byte into the shared CLI command owner;
query consumers import that owner directly.

Input parsing, quote validation, arity and value limits finish before database
open. Source authorization and the common fresh-copy publisher retain encrypted
and deployment refusals, complete-file sync and no-overwrite behavior. This
creates a separate non-journalable engineering copy; it does not grant normal
ACE saves, required macro execution or directory-entry durability.

The independently frozen corpus has 16 pairs/32 images covering two and three
rows, both ACE 16 headers, ordinary/generated insertion and literal/clock
omissions. SOURCE passed before consumers. Three parser controls and the prior
quoted-TSV test pass. Four API controls pass: 24 complete CLI copies, 16 exact
clock-service images and four fully supplied old-path copies, plus late
conflict/invalid-presence/missing-clock refusals. The first SOURCE compile's
module-path correction changed no fixture bytes. See the [execution record](results/ace16-typed-batch-omissions-2026-10-30.json).
The broader run passes: native LVAL15 and Extra41; Access63 library functions
plus six integration functions; CLI43 unit functions plus 107 integration
functions across 12 targets, including INSERT, clock and eval. Minimal ACE/Access,
formatting, documentation links and dependency boundaries pass. Final strict
checks pass for all ACE/Access/CLI targets and features after an independent Memo
fixture qualification correction; the earlier failure remains recorded.
