# Exact ordinary Single/Double updates

The ordinary fixed UPDATE service has an exact Single/Double continuation for
unindexed native fields. It uses complete original Source admission, the shared
fixed-record stage, independent whole-image reconstruction and the existing
checked V1 journal lifecycle. It does not convert an engineering preview into a
save or accept a supplied absence certificate.

The engine and retained typed Access service carry exact `Single` and `Double`
Values. Original expected values compare by representation: negative zero,
subnormal values and NaN payloads retain their bits. An integer, Double or Text
value is not implicitly a Single assignment. SQL must supply the appropriate
stored carrier through its separately admitted expression rules; SQL equality
and formatted output do not replace expected-value bit checks.

Named [Byte/Int16](NORMAL_BYTE_INT16_FIXED_UPDATE.md) targets select cumulative
TCNI9; named [GUID targets](NORMAL_GUID_FIXED_UPDATE.md) select cumulative TCNI10.
Each later dialect retains exact IEEE bits while old TCNI8 remains closed to the
additional target families, including empty groups.

## Native target and preservation checks

Every named target qualifies before no-op filtering, including a group with no
row assignments. The native type and shared scalar type must agree, the field
must be fixed with flags 3 and extended flags 0, and storage must be exactly four
bytes for Single or eight for Double. Existing overlap, generated-field,
dependency, program, constraint, row-witness and ownership checks remain. All
requested fields in this continuation must be unindexed; mixed requests that
also mutate an indexed sibling refuse before staging. Unchanged fields and
unrelated native storage do not grant authority to edit another target.

The writer uses the actual shared scalar encoder. Single occupies four meaningful
bytes in the existing eight-byte staging cell; Double occupies eight. The
independent proof derives raw little-endian bits from original typed requests,
checks original logical NULL masks and expected values, and reconstructs the
complete original image. It consumes neither staged patches nor producer counts.
A fresh canonical AFTER Source verifies final records and preserved owners.

The qualified source pair laws change only the requested IEEE payloads and,
in one pair, the explicitly requested existing DATE payload. Record lengths,
physical ROW references, masks, slot words, counts, siblings, maps, catalog,
protected bytes and EOF remain unchanged. No allocation, record growth or index
rewrite is introduced. Positive byte-oracle coverage for nullable IEEE transitions
and mixed variable/IEEE assignments remains separate work toward general writes.

## Original intent and recovery compatibility

The existing consuming API is
`NativeImage::plan_recoverable_ordinary_fixed_updates`. Exact IEEE original groups
use `NativeFixedIntentGroup::new_ordinary_ieee`; construction retains data and
spelling, without admitting a native file. Original metadata selects the request
dialect before comparisons. Naming an IEEE target selects
`NativeUpdateProfile::Ace16OrdinaryIeeeFixed`, even for an equal or named-empty
group. Requests also naming Byte/Int16 use the cumulative
[TCNI0009 continuation](NORMAL_BYTE_INT16_FIXED_UPDATE.md); legacy-only ordinary
requests retain TCNI0004 encoding.

`TCNI0008`/profile 8 reuses the fixed original-group framing, BEFORE length and
digest, bounded names/counts and nine-byte carrier cells. Tags 0 through 5 retain
the existing NULL, Boolean, Int32, Int64, Currency and DATE representations.
Single tag 6 stores four little-endian raw IEEE bytes followed by four zero
padding bytes; Double tag 7 stores eight raw IEEE bytes. Noncanonical padding,
unknown tags and mismatched magic/profile refuse. Legacy TCNI0003/4 records retain
their closed carrier grammar and do not decode the new tags. No source capability,
final row, permission flag or staged byte patch is serialized.

Recovery chooses exactly the original record's validator and redoes complete
Source, target, expected-value and independent byte checks against the journal's
BEFORE image. It compares both complete images before effects and retains the
same partial/committed mismatch and durable namespace rules. A failed decoder or
planner is never retried as another profile. Positive BeforeChange, fitted TCNI5,
owned-growth TCNI6 and EOF TCNI7 carrier policies remain unchanged.

## Evidence and current verification state

The [execution record](../../../tests/results/rows/ace16-normal-ieee-update-2026-11-03.json)
separates source admission, matching writes and retained regressions.

Six unchanged independently authored native17 images from the prior wide-scalar
INSERT source corpus supply four UPDATE pairs across both headers. They are
native derivatives, not SDK-created originals or writer-produced expected
UPDATE images. The complete read-only gate passed six exact reads, six
NoRequired admissions, six canonical Source admissions and four protected-byte
pair reconstructions before matching UPDATE execution. The original
[wide-scalar source record](../../../tests/results/rows/ace16-wide-scalar-insert-sources-2026-11-01.json)
remains historical source evidence.

The new three primitive tests and eleven existing fixed-intent tests passed,
as did reader-only, journal-only and preview-only feature builds. The matching independent proof, native recovery/resource, retained typed and CLI
tranche also passes: four literal whole-image commits and typed saves, forty
direct protected-byte forgeries, twelve bit-distinct stale witnesses, twenty
no-op/named-empty requests and sixteen recovery contexts. Two additional CLI
saves use manually checked finite-zero outcomes. A separate legacy named-empty
control retains old TCNI4 decoding while refusing IEEE native targets before
effects. One hundred retained engine/host/index regression functions pass,
followed by scoped strict all-target checks, standalone reader/journal/preview
builds, dependency guards and formatting. The first wider-sibling regression
runner used the wrong fixture directory; resuming with the retained directory
changed no source, expectation or production code. These matching results remain
separate from the earlier source gate.

The matching corpus targets exact IEEE bit patterns through typed requests. The
CLI tranche separately checks finite zero constructors `CSng(0)` and `CDbl(0)`;
it does not claim SQL construction of the subnormal or NaN source values.
Native Microsoft Access save/reopen, Windows execution, mixed-client behavior
and power-loss qualification remain separate and unrun.
