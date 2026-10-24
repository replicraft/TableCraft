# Explicit ICU host text profile

`query run --text-locale` supplies a working, opt-in linguistic provider for
StrComp text mode and InStr/InStrRev/Replace text searches. It accepts the closed
locale set en-US, de-DE, sv-SE and tr-TR. No OS locale is read. These are ICU host
profiles; they do not establish Windows NLS, Jet/ACE database or native Access
query equivalence. Native General field/database providers retain their existing
separate implementation and qualification limits.

```sh
tablecraft query run \
  "SELECT StrComp('A','a',1),InStr(1,'😀AéZ','É',1),Replace('ﬁlms','fi','X',1,-1,1)" \
  --text-locale en-US

tablecraft query run "SELECT InStr(1,'xI','ı',1055)" \
  --text-locale tr-TR --text-lcid 1055
```

The second command explicitly registers that selected profile for exactly LCID
1055. The library/CLI does not infer numeric language IDs, alternate sorts or other
aliases from a locale name. Without registration, numeric LCID dispatch remains
unavailable. Registering one complete LCID does not register its base/alternate
sort identities or qualify native sort semantics. Hosts own these mappings.

## Comparison and search contracts

ICU secondary strength ignores case, retains accent differences and enables
canonical normalization. Case level is off and punctuation is nonignorable.
The profile has no numeric-ordering or arbitrary locale-extension switches.
English/German/Swedish/Turkish locale data remain distinct: Swedish places ö
after z, while German places it before z; Turkish distinguishes I/i while pairing
I/ı and İ/i. No SQL source is scanned to select these settings.

Substring search enumerates nonempty Unicode scalar-boundary spans and compares
each complete span under the selected ICU profile. Forward search chooses the
least start; reverse search chooses the greatest admitted start. Equal matches
at a start choose the shortest consumed span. Returned offsets measure original
UTF-16 units, including both units of supplementary scalars; expansions and
canonical equivalents retain their original consumed width. For example, searching
FI for ﬁ consumes two original units, while searching ﬁ for fi consumes one.
These explicit span/tie rules are not claimed to be native NLS search behavior.
The implementation is a bounded correctness-first search, not a fast collation
search index or a universally linear algorithm.

Text mode accepts decoded Unicode. Unpaired/odd raw Strings fail strict publication
before this provider; no replacement characters or binary retry appear. Binary
string modes retain their existing raw-unit behavior. Database compare, SQL
predicates, field-policy resolution, LIKE and database search delegate to the
selected base provider unchanged. Supplying only --text-locale does not activate
ordinary SQL comparison, LIKE or an omitted Option Compare policy. Explicit
--collation can be composed with the text provider; it retains its separate policy.

## Limits, lifetime and dependencies

The optional `tablecraft-expressions/icu-text` feature owns IcuTextLocale,
IcuTextLimits and IcuTextCollation under `comparison::icu_text`. Normal CLI builds
enable that feature; standalone expression builds need not. The implementation
uses ICU4X Collator compiled data, not platform NLS/COM/VBA or locale discovery.
Hosts may compose a borrowed base Collation and explicitly register one LocaleId.
No provider, registration or mutable quota is stored in a prepared query or file.

Each atomic ICU comparison admits at most 65,536 UTF-8 bytes per operand. Search
additionally admits at most 4096 UTF-16 units in each operand. Hosts may lower
these validated ceilings; large query/native/output allowances cannot enlarge
them. Input traversal work is prepaid before each indivisible ICU comparison,
with cancellation checked before and after. That bounded call cannot be interrupted
internally. Search checks cancellation throughout enumeration and charges every
comparison to one cumulative provider pool. It can exhaust work before finishing
a larger search; it never publishes a partial match or retries another policy.

Owned search-boundary vectors reserve/charge storage fallibly before filling.
ICU itself uses internal scratch buffers; this wrapper does not replace ICU's
allocator with the caller's fallible budget. The hard operand ceiling bounds
upstream scratch input, while provider step units are a documented admission
model, not an exact ICU element/allocation count. This limitation remains explicit.

CLI --comparison-bytes and --comparison-steps may accompany --text-locale without
--collation. Each distinct General/ICU provider owns its own cumulative allowance;
the knob does not create one cross-provider pool. Defaults remain 16 MiB and one
million steps, with maxima 256 MiB and one hundred million. Query execution,
native reads, Text values and output retain separate budgets. CLI results remain
buffered on provider failures.

## References and checks

The primary [ICU4X Collator API](https://docs.rs/icu_collator/2.3.1/icu_collator/struct.Collator.html)
and [options](https://docs.rs/icu_collator/2.3.1/icu_collator/options/struct.CollatorOptions.html)
define the selected secondary/case/punctuation profile. ICU4X does not provide a
substring matching API; the bounded span enumeration is TableCraft code.
Microsoft's [NLS sorting guidance](https://learn.microsoft.com/en-us/windows/win32/intl/handling-sorting-in-your-applications)
describes distinct linguistic/ordinal and locale-sensitive behavior. It does not
establish that ICU weights equal NLS or the database's persisted sort weights.

Seven provider tests cover documented ICU secondary examples, locale contrasts,
canonical/ligature consumed spans, exact LCID registration, base delegation,
input ceilings, cumulative quotas and every observed cancellation checkpoint.
A seeded property test uses an independent ASCII caseless-position model.
Six CLI tests cover explicit text operations, locale/LCID selection, binary/raw
refusal, native database field resolution with automatic indexing, budgets and
pre-I/O option validation. Existing General/database and comparison-limit tests
remain applicable. Native Access comparison remains unrun; wider LCID providers,
NLS error/ordering equivalence and mixed database-policy precedence remain P0.

The original [ICU4C comparison runner](testing/compare_icu_text.py) passes **3,032
controls** against a fixed CLI copy. It calls the separately installed ICU4C 74.1
ucol_strcoll implementation, with explicit matching secondary/case/normalization/
punctuation/numeric settings. Pairwise anchors, seeded mixed text, canonical and
ligature spans, supplementary offsets, forward/reverse bounds, NULL-byte strings
and explicit per-locale LCID registrations cover all four profiles. Expected search
positions come from independent Python scalar-boundary enumeration with ICU4C
comparison; this is not an ICU4C StringSearch or Windows NLS search equivalence test.

The [recorded report](testing/results/icu-text-profile-2026-10-24.json) retains the
runner/input/CLI hashes and ICU4C library/collation versions. The CLI hash is
unchanged before/after. All inputs are original authored/generated text; no native
database or third-party fixture is created or copied. Library version differences
are allowed to surface as failures, not silently accepted as matching data. Actual
Access and Windows NLS remain unrun.

```sh
python3 docs/testing/compare_icu_text.py \
  --cli path/to/fixed/tablecraft \
  --report path/to/fresh-icu-report.json
```

The recipe uses an available ICU4C library, with --icu-library for explicit
selection; no system package is installed or downloaded. Its C ABI calls follow
the upstream [ICU4C collation contract](https://unicode-org.github.io/icu-docs/apidoc/released/icu4c/ucol_8h.html),
and the UTF-16 bridge currently requires a little-endian host.
