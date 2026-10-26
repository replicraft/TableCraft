# Known non-Text concatenation policy replay

The [accepted replay](results/concatenation-policies-2026-10-24.json) passes
300 commands and all 1,882 unchanged hashes, including every entry in the frozen
1,861-file Rust inventory. It reuses the four registered original native-policy
fixtures for Jet 4, ACE 2007 and ACE16-family V2016/V2019.

The candidate passes 164 controls: 112 successes with 116 modeled result cells,
and 52 classified buffered refusals. The fixed older CLI supplies 136 paired
controls: 28 successful cells and 108 expected refusals. Exactly 76 former
policy-conflict refusals become successful modeled results; the other 60 paired
outcomes remain unchanged (28 successes and 32 refusals). Auxiliary candidate
controls cover an explicit index, global override and failure limits.

These are actual source-policy routing and independent ASCII identity results,
not native Access comparison/precedence qualification or a new reference weight
model. Existing NULL and actual String provenance rules remain conservative.

## Getter and model boundary

[ConcatenationPolicyOracle.java](ConcatenationPolicyOracle.java) reopens the
registered original files read-only, disables expressions and installs a throwing
link resolver. Public row getters capture exact stored text; pinned Jackcess
ColumnImpl/DatabaseImpl metadata getters capture field/default sort identities.
No reference SQL, expression, sort-weight encoder or imported content executes.
The getter verifies that no linked handle was opened.

The registered [NativePolicyFixture.java](NativePolicyFixture.java) originally
created LegacyText with LCID1033/version0 and ModernText with LCID1033/version1.
Both have variant0. Row3 contains exact `a` / `A`; the remaining ordinary rows
include Unicode, NULL and trailing-space inputs. Original file hashes remain:

| Profile | SHA-256 |
| --- | --- |
| V2000 | `9aa2b386d739e99228ac6ed82e772dc073f3094d038d32a08cb0785718d5c4a2` |
| V2007 | `431835e835186515df4ab11673aef860741c935ad0867dc73166f577e4688c28` |
| V2016 | `86076051e896e0e79c5e2e649f4dfd5e0386e6a87ef145314730901f8454ee6e` |
| V2019 | `3ae803876edee6a5510169ebbaa380a8a1dbe1a522edf4b080d7047ea5ce3347` |

[compare_concatenation_policies.py](compare_concatenation_policies.py) asserts
those exact registered hashes and getter identities before replay. It independently
constructs short ASCII outputs from the raw `a` value and explicit rendering
goldens: `2`, `1.25`, `True`, `2000-01-02` and `12`. Equality compares each
result with the identical expected String; no second linguistic equality oracle
is needed. The suite separately requires the documented conservative refusal
when actual String contributors retain incompatible native policy identities.

Examples include `LegacyText & IIf(True,2,ModernText)` and the reverse direction,
which contain an actual numeric operand with a latent policy from the conditional
String alternative. Under the explicit declaration profile, admitted String `+`
construction has the same problem. Known numeric, Boolean and Date values now
cease contributing that latent Text policy. Their formatting/data results stay
unchanged. Both materialized/declaration profiles cover `&`; declared and retained
default behavior are checked separately for `+`.

The same correction survives derived queries, UNION with a literal arm, Min,
DLookup and DCount criteria. The count criterion includes `ID=3 AND ...`; eager
comparison still executes on other rows, while the independent ID condition
makes only the identical row3 String contribute to the expected count.

## Retained boundaries

Actual LegacyText/ModernText String contributors still conflict under stored
policy selection. An empty actual-String UNION arm retains its declared policy
contribution and still refuses; an empty arm is not silently discarded from
schema authority. An explicit global ordinal override remains available.

NULL field and conditional-result provenance stays conservative because NULL
alone has no reliable origin discriminator. Therefore
`LegacyText & IIf(True,NULL,ModernText)` remains a policy conflict in this slice.
Bare literal NULL, NULL-propagating `+`, fresh CStr output provenance and default
numeric `+` type failures retain their existing outcomes. CStr applied even to a
selected actual String remains a fresh output under the existing contract; this
replay does not change that behavior.

Missing numeric formatting, eager division errors and positive execution/output
limits require their specific diagnostic classes and empty stdout. This is not
exhaustive resource/cancellation qualification; service tests retain those
contracts. No new native carrier, stored-policy precedence rule or strict
set/group/sort coercion is inferred.

## Provenance and reproduction

The existing binary fixtures remain under ignored
`.cache/tablecraft-fixtures/native-policy-20261024-provenance/` and were never
patched or regenerated for this slice. The new getter captures and actual getter
commands are under `.cache/concat-policy-getters-20261024/`, with generation.json
recording each unchanged original hash. Existing fixture provenance remains in
[the ledger](FIXTURES.md).

The reviewed Jackcess 5.0.4 JAR is Apache-2.0, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
The original getter/runner follow the repository MIT license. No third-party
implementation or fixture bytes are copied, and the existing weight-model oracle
is not called by this replay.

```sh
javac -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  -d .cache/concat-policy-classes-20261024 docs/testing/ConcatenationPolicyOracle.java
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/concat-policy-classes-20261024 \
  ConcatenationPolicyOracle \
  .cache/tablecraft-fixtures/native-policy-20261024-provenance/Policies-V2016.accdb
```

Repeat read-only getters for all four profiles, capturing stdout/stderr as
`V2016.getters.tsv` / `V2016.getters.stderr` and similarly for the others.
The frozen artifacts are:

- Candidate `.cache/tablecraft-bin/concat-policy-cli-20261024`, SHA-256
  `747425183d2e0869646fa359f3f557a540bff6fb835116a8912e6346a38c4cf3`.
- Baseline `.cache/tablecraft-bin/external-named-cli-20261024`, SHA-256
  `8b9c8e89f6804f57e45a6bba74cf7b6ebbdb8b7a20917cd8b1170c6e31710d23`.
- Source `.cache/concat-policy-rust-source-20261024.manifest`, SHA-256
  `cf5c9a255d44634b31a91fb74192590b56053749b504914d992e047e07f72ba2`.

```sh
python3 docs/testing/compare_concatenation_policies.py \
  --cli .cache/tablecraft-bin/concat-policy-cli-20261024 \
  --cli-sha256 747425183d2e0869646fa359f3f557a540bff6fb835116a8912e6346a38c4cf3 \
  --baseline .cache/tablecraft-bin/external-named-cli-20261024 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/native-policy-20261024-provenance \
  --getters .cache/concat-policy-getters-20261024 \
  --source-manifest .cache/concat-policy-rust-source-20261024.manifest \
  --manifest-sha256 cf5c9a255d44634b31a91fb74192590b56053749b504914d992e047e07f72ba2 \
  --report docs/testing/results/concatenation-policies-2026-10-24.json
```

The runner refuses an existing report and verifies every source-manifest entry
before/after, along with binaries, input/getter/source files. Accepted JSON
retains all commands, outputs, diagnostic classifications and baseline pairing.
The terminal log is `.cache/concat-policy-final-20261024.log`.

Exploratory results stay private and are excluded from accepted counts. An early
harness assertion projected raw Text while expecting a comparison conflict; the
final suite explicitly requests comparison for those cases. An initial proposed
84 corrections included eight conditional-NULL cases; they correctly remain
refusals and are excluded from the final 76 corrected pairs. Existing CStr fresh
provenance was also confirmed rather than changed. The exclusion record is
`.cache/concat-policy-exclusions-20261024.md`; no original inputs were modified.

Native Access collation/precedence evidence and broader field-origin semantics
remain separate; see [native Text policies](../access/indexes/NATIVE_TEXT.md).
