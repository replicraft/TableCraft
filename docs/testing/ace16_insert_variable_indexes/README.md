# Variable-key secondary INSERT evidence

This repository-authored corpus has 42 raw-source-qualified V2016/V2019
originals: 38 positive engineering candidate cases and four intentional native
refusal cases. Raw/public-writer qualification does not itself grant native
write admission. Each table has manual required Int32 ID, nullable Int32 Group
and nullable Binary or Text Payload. Four logical names share two physical
owners, with a required unique ascending ID primary.

The finite scope covers Binary and General Legacy/General text single keys in
ASC/DESC order, mixed Payload/Group tuple orders and directions, short unique
IgnoreNULL tuples, Binary pressure and Modern whole-entry digests. Text has an
explicit AllowZeroLength=true declaration and uncompressed storage. The closed
text alphabet is a/z, including empty and 255-unit values; these observations do
not qualify every General character weight or SQL collation behavior.

Actual Text255-unit records carry 510 raw UTF-16 bytes. Public Jackcess 5.0.4
limits Binary declarations to255 bytes, giving 289-byte full Binary keys before
ROW. Authentic Binary510/577-byte key qualification remains open; existing
literal engineering coverage is separate and no source/schema bypass closes
that gap. NULL, present empty, partial-NULL and ignored all-selected-NULL tuples
remain distinct in records, keys, membership and historical counters.

The public source/reference experiment has preserved failure lineage:

- The first compilation failed because setTextSortOrder returns void. A fresh
  corrected attempt failed because the metadata helper had not propagated the
  checked IOException from getProperties. Neither attempt generated a database.
- The third compilation succeeded. All 42 raw source gates passed before any
  public append. After 133 successful child calls, the Python proof stopped on
  the second completed reference: it had compared complete KEY+ROW strings
  instead of comparing key bytes first and logical ROW separately.
- A corrected saved-only replay passed all 42 sources and both completed
  references. The original proof and unexecuted model remain archived. A frozen
  continuation executed only the remaining 120 calls, completing all 42 public
  transitions without repeating a completed source, append or reader call.

Thus there are 253 distinct completed planned calls plus two failed compilation
calls, not an overall first-pass source/reference result. The original stopped
report remains completed=false; no combined successful raw report is invented.
Eighteen known omitted-NULL warning events retain their exact checked grammar.
Public allocation/framing is observed independently of the engineering policy.

All 38 complete positive images were derived only from requalified SOURCE bytes
and exact requests and frozen before the isolated committed-source build. The
model independently encodes Binary segments, finite text weights, whole-tuple
digests, actual-layout records and DATA-first allocation. It consumes no public
AFTER image or Java NEW encoder stream as construction authority. Its comparator
separates KEY and ROW, including branch bounds after child-pointer removal.

The first 38 producer calls matched every complete image. The four first guard
calls each returned exit1, empty stdout and exactly Unsupported(ConstraintCoverage),
with no output file: a unique Modern source digest is not exact enough to qualify
physical uniqueness. The producer includes the independently identified KEY/ROW
audit fix. All positive images and all four guard outcomes were checked before
the first 76 fresh read-only getter/map calls, which passed. Positive cases
preserve 206 old ROWs, insert 170 rows and contain 376 final rows and 734 physical
key/ROW pairs. They allocate four DATA pages and eight INDEX pages.

Run the saved replay from the repository root:

```sh
python3 -B docs/testing/ace16_insert_variable_indexes/verify.py
python3 -B docs/testing/ace16_insert_variable_indexes/verify.py --verify-git
```

Default replay starts no child process. The explicit Git option starts one
metadata batch binding the full archived producer source and committed receipt;
neither mode invokes a producer, Java or fixture generation. Missing ignored
evidence returns available=false and exit2 without creating replacements. Use
--evidence-root PATH for a complete relocated evidence tree. Hash reads of raw
artifacts can occur during closure checks; semantic public AFTER replay follows
all38 expected-image derivations, and fresh getter interpretation follows all38
candidate comparisons and four exact refusal checks.

recipes.py and the Java helper retain exact private bytes. reference.py and
stderr_policy.py change only repository bootstrap and owning recipes imports;
all function ASTs and exact transforms are checked. model.py retains its complete
constructor body and actual maintained multiple-index framing owner, changing
only its repository root. Frozen executed scripts, models, reports, source
images and expected images remain untouched in ignored evidence.

No third-party fixture or implementation source is copied into this package.
See the [compact result](../results/ace16-insert-variable-index-native-2026-10-28.json)
and [fixture provenance](../FIXTURES.md). These finite engineering and independent
reader observations do not qualify native Microsoft Access saves/reopens, data
macros, Windows integration or normal production write paths.
