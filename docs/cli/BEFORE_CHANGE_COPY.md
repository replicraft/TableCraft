# Explicit BeforeChange copy

`query preview-before-change` creates one fresh experimental fitting copy from a
local one-column UPDATE and explicitly selected BeforeChange properties. Choose
the exact KIND/NAME keys through inert `inspect --table TABLE --extra` metadata;
`StoredGuard` below is a fixture property name, not automatic event discovery.

```sh
tablecraft query preview-before-change 'UPDATE Items SET Score=99 WHERE Id=2' \
  --database offline.accdb --output proposed.accdb \
  --guard-property 3 StoredGuard --trace --durable-output
```

Property order is caller engineering order. This command does not certify native
event dispatch, required-program coverage, general macro compatibility, normal
ACE saves or journal permission. Ordinary `preview-update` keeps its active-content
refusal. The existing read-only `prepare-guards` path keeps its original selected
guard semantics and creates no candidate.

The CLI parses and checks the local UPDATE shape/selectors before source/output
capture. One immutable source/snapshot and the shared Access service resolve all
SQL requests, execute the qualified selected programs and form one joint final
fitting candidate. No candidate chaining, defaults, generation, cascades or DATA/
INDEX growth is introduced. The current physical profile is ordinary Int32/NULL
rows under all native source/constraint/ownership checks. Empty selection still
compiles every selected original program. RaiseError returns no candidate and
discards earlier logical changes atomically. The CLI reports its numeric error,
not a raw stored description.

`--trace` shows only visited control/assignment positions and decoded XML UTF-8
spans. It prints no XML, expression or row values. Program names are escaped.
Matched rows describe original SQL selection; changed rows describe distinct
jointly final row changes. Assignment counts are executed SetField locations,
including assignments whose final values equal the originals.

Reports are budgeted and buffered before fallible source release and publication.
The one common publisher owns the full output-byte charge. Existing outputs,
source aliases and symlinks are not replaced. Optional Required publication is
selected once before source I/O and never falls back; success acknowledges the
captured-directory barriers. Encrypted/deployment inputs refuse without a
plaintext downgrade or selected-program execution.

Publication errors retain the IO owner's not-published, uncertain, visible and
cleanup/barrier facts. After successful publication, a report write/flush failure
retains the known published-copy fact. Diagnostic BrokenPipe cannot convert those
states to success. Input/preflight errors retain invalid-input status; other old
commands keep their output behavior. A final output can exist after a visible or
uncertain error; no rollback, final-entry removal or automatic retry occurs.

Default work allowances are 268435456 bytes, 100000 rows and 100000000 steps.
Standard complete-image/structural caps remain unchanged. Reports have a separate
one-megabyte bound and fail before publication when exceeded. No image-size,
variable, LVAL or cascade mode is implicitly selected. The local record covers
macOS; implemented portability is not a Windows/Linux native Access comparison
or physical power-loss qualification.

The exact existing eight physical source/expected pairs were reread before new
host APIs. First bin5/main1/API3 logs pass: twelve primary complete copies plus
four empty no-op copies; four late RaiseError, twelve occupied-output, four
missing/duplicate-selector and two unchanged-old-profile refusals. Focused units
cover preopen shape/selector limits, write/flush BrokenPipe, publication state
through failed diagnostics and source-release blocking both publication modes.
An additional SOURCE test passes genuine encrypted correct-password reads and
checks plus exact stored/MSZIP deployment extraction. Its subsequent API passes
eight no-output refusals: encrypted input without and with the correct password,
and stored/MSZIP packages, each under default and Required publication. All
original envelope bytes remain exact and no output or staging artifact remains.
These repository-authored envelopes establish the selected reader profile, not
native Access encryption comparison.

Broader checks pass 76 CLI binary units, 20 guard integration functions and eight
journal integration functions. Combined ACE/Access/CLI all-target/all-feature
strict checks pass. Final ACE/Access/CLI formatting and ACE/Access no-default
builds pass, along with 6718 local documentation targets (none missing), 15
package-boundary checks and 12 policy tests. No product test failed in this host
round; import-only review and additive envelope registration changed no source
constructors or expected bytes. See the [pinned result](../testing/results/ace16-before-change-cli-copy-2026-10-30.json).
