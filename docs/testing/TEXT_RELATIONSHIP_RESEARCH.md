# Text foreign-key comparison research

Reviewed 24 October 2026. Text foreign-key membership remains Unsupported.
This record explains a concrete reference limitation; it does not qualify a new
reader, comparer, writer or native Access behavior.

## Unique keys and foreign-key matching differ in the reference

The original [fresh-file probe](TextRelationshipProbe.java) creates separate
parent/child and unique-probe tables using the reviewed Jackcess 5.0.4 artifact.
It disables expression evaluation. Parent values include Alpha, accented é,
Straße and empty Text. Each file contains both explicit General legacy and
General 2010 field descriptors. Unique-probe inserts occur in a separate table,
so a successful probe never changes the actual foreign-key parent set.

The following outcomes repeat across original Jet 4 and ACE 2007/2016/2019 files
under both General versions:

| Existing parent | Attempted value | Unique-probe insertion | Child insertion |
| --- | --- | --- | --- |
| Alpha | alpha or ALPHA | Rejected as duplicate | Accepted |
| Alpha | Alpha plus trailing space | Rejected as duplicate | Rejected as missing parent |
| Alpha | Alpha plus U+0000 | Rejected as duplicate | Rejected as missing parent |
| é | É | Rejected as duplicate | Accepted |
| é | e plus U+0301 | Rejected as duplicate | Rejected as missing parent |
| Straße | Strasse | Rejected as duplicate | Rejected as missing parent |
| Empty Text | Empty Text | Rejected as duplicate | Accepted |
| No matching non-NULL parent | NULL | Accepted | Accepted as all-NULL exemption |
| No matching parent | e or missing | Accepted | Rejected as missing parent |

The reference's [FK enforcement source](https://jackcess.sourceforge.io/xref/com/healthmarketscience/jackcess/impl/FKEnforcer.html)
selects its [case-insensitive column matcher](https://jackcess.sourceforge.io/apidocs/com/healthmarketscience/jackcess/util/CaseInsensitiveColumnMatcher.html).
These primary implementation references help explain the observed distinction;
they do not establish Microsoft's native matching rules. Conversely, existing
physical text-key encoder comparisons cannot qualify native foreign-key equality
merely because the index bytes agree.

Microsoft's [relationship guide](https://support.microsoft.com/en-us/access/guide-to-table-relationships)
describes matching key fields and enforced membership. Its
[Field.CollatingOrder contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-collatingorder-property-dao)
describes stored comparison/sort settings. These do not resolve the exact
padding, ignored-unit, canonical-equivalence and expansion behavior exposed here.
Native qualification remains necessary before treating either reference model
as complete Access text-FK semantics. No OS locale or generic ICU substitute is
inferred.

## Recorded scope

The [research report](results/text-relationship-research-2026-10-24.json) records
176 reference insert attempts across four formats and eight TableCraft audits.
The audits retain Unsupported for both Text relationships per file and report
five retained child rows per relationship. Original database hashes remain
unchanged during the read-only audits. This is diagnostic evidence, not 184
passing Text-FK compatibility tests.

The report pins the original producer, reference JAR, immutable existing CLI,
four databases and four complete tagged TSV logs. Text values in logs use UTF-8
hex with a distinct NULL tag, retaining the empty/NUL distinction. No library
implementation or external fixture is copied. The
[fixture ledger](../FIXTURES.md#original-text-relationship-research-inputs)
records hashes and excluded capability probes. Native Access comparisons remain
unrun; required write validation is not relaxed.

To regenerate, choose fresh output names and run:

```sh
java -Dstdout.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar \
  docs/testing/TextRelationshipProbe.java fresh.accdb V2016
```

The same producer accepts the recorded V2000, V2007 and V2019 format names.
Run explicit `inspect --table Children0 --constraints` and `Children1` audits
against copies or unchanged originals; these commands do not execute stored rules
in this probe because none are declared.
