# Independent JDBC aggregate metadata observations

The [four-profile report](results/jdbc-aggregate-metadata-final-2026-10-25.json)
records 58 queries over each original Jet 4, ACE 2007, ACE 2016 and ACE 2019 scalar
fixture: 232 UCanAccess queries and 232 TableCraft queries. It keeps JDBC declared
metadata, actual Java object classes/values and TableCraft VarType/TypeName/display
values in separate fields. It is an observation matrix, not a native Access oracle
or a conformance pass/fail comparison between different numerical profiles.

All TableCraft queries complete. The reference completes 224 queries and returns
eight retained execution errors: First/Last on the fixture's WideReal field in
each profile. It reports invalid-character conversion rather than a successful
value. These errors remain in the report; no query or source value was removed to
turn them into successful controls. Ten immutable artifact/input/tool hashes and
all four working-copy hashes were independently rechecked.

## Findings that affect the next implementation decision

| Expression | Independent reference observation | TableCraft observation / decision |
| --- | --- | --- |
| Count, DCount and integral Sum | BIGINT / java.lang.Long; Sum of two Long maxima is 4294967294 | Existing Int64 profile agrees; this does not justify narrowing it to Int32 |
| Avg of IDs 1 and 2 | Actual java.lang.Long value 1, despite INTEGER/java.lang.Integer metadata | Existing Double result 1.5 remains; do not substitute the reference's integral-average profile |
| Avg of all IDs | INTEGER/java.lang.Integer metadata with an actual java.lang.Long value 2 | Metadata and actual carrier identity must remain separate |
| Sum/Avg of Money | BigDecimal values with DECIMAL metadata | This does not identify a native Access Currency versus Double expression result |
| Avg of the exact Amount inputs | Reference retains four fractional digits; TableCraft retains the exact additional half digit | Preserve the separate existing rounding/scale contracts pending native evidence |
| First/Last WideReal | Reference execution errors in all four profiles | Retain errors as limitations of this reference route, not unsupported TableCraft functions |

JDBC precision/scale/nullability are exactly what this driver reports. For example,
Count's reported precision is 64 and nullability code is 2 (unknown); these are
not native Access column constraints or a Scalar/Variant expression declaration.
No runtime carrier, result inference, aggregate category or default arithmetic
behavior changed as a result of this probe. Full native return-type/promotion
qualification remains open.

## Reviewed external artifacts

These are optional engineering tools, outside Cargo and ordinary builds:

| Artifact | License review | SHA256 |
| --- | --- | --- |
| io.github.spannm:ucanaccess:5.2.0 | Apache-2.0, embedded NOTICE retained | ed8f33f30b9d08c7dc0bead139180ea70061beea915677ce2fe47e4b7aea2490 |
| io.github.spannm:jackcess:5.2.0 | Apache-2.0 in its published POM | 9451d6f336cfaa82f5af178d5f6668b933cda4f27e261f29a1faa83630f95775 |
| org.hsqldb:hsqldb:2.7.4 | HSQLDB/Hypersonic BSD-style terms | 5fab2bb4384ac06b762638c8fa2740c944b8d080e4796c0c6c2af8b90dd4e5ad |

Artifacts came from the official Maven repository. Published SHA256 sidecars were
verified for UCanAccess and Jackcess. HSQLDB does not provide a SHA256 sidecar at
the tested URL; its published SHA1 was verified and a local SHA256 pinned. This
records retrieval provenance, not a stronger authenticity guarantee. JARs and
third-party sources remain ignored; none are vendored into the product.

The published POMs identify HSQLDB and Jackcess as UCanAccess dependencies. Optional
Apache POI is not enabled, and test/provided dependencies are excluded. This
three-JAR runtime compiled and executed successfully; inspecting an SLF4J POM did
not add an SLF4J runtime dependency. The UCanAccess source JAR was consulted for
the public opener contract, without copying implementation code.

Primary references: [UCanAccess project/license](https://github.com/spannm/ucanaccess),
[versioned UCanAccess POM](https://repo.maven.apache.org/maven2/io/github/spannm/ucanaccess/5.2.0/ucanaccess-5.2.0.pom),
[versioned Jackcess POM](https://repo.maven.apache.org/maven2/io/github/spannm/jackcess/5.2.0/jackcess-5.2.0.pom),
[HSQLDB license](https://hsqldb.org/web/hsqlLicense.html), and
[connection/opener documentation](https://spannm.github.io/ucanaccess/20-getting-started.html).

## Inert input and execution boundary

The Java probe and Python runner are original Apache-2.0 repository code. The
runner accepts only four exact registered original hashes, creates fresh copies,
and compares all original/copy hashes after execution. The custom trusted opener
forces Jackcess read-only, disables expression evaluation and authorizes only the
one canonical input copy. Inert metadata preflight refuses linked tables and stored
query definitions before JDBC initialization. UCanAccess replaces an opener's link
resolver, so the guard does not rely on that replaceable callback. A separate
original application-copy control with stored queries refuses before any QUERY
output, with original/copy bytes unchanged; the
[probe review](results/jdbc-aggregate-probe-review-2026-10-25.json) records it.
JDBC operates with an in-memory mirror, read-only connection and a
hardcoded SELECT list; no document scripts, writes, providers or new databases
are requested. SQL timeouts, result-row limits and a 256 MiB JVM heap bound the
probe. This external reference is not an application dependency or JVM backend.

The first 57-query diagnostic and its source/output remain preserved privately.
A fractional-integer-average control was then added. The first complete
four-profile matrix is also preserved: naming resource ceilings afterward changed
no SQL, runtime observations or errors, as checked against the final matrix.
The later canonical-target/metadata guard likewise leaves every recorded query
observation unchanged. These are reference observations, not excluded failing
TableCraft tests.

Reproduce with reviewed JARs and a fresh work directory/report:

```sh
python3 docs/testing/compare_jdbc_aggregate_metadata.py \
  --artifacts .cache/ucanaccess-reference-20261025 \
  --inputs .cache/tablecraft-fixtures/scalar-seek-20261023 \
  --cli .cache/tablecraft-bin/progressive-stateful-cli-20261025 \
  --workdir FRESH_WORK_DIRECTORY --report FRESH_REPORT.json
```

The TableCraft CLI is frozen at committed revision
`57140e145eaef59b3de6f8b84eca6852a8526ea8`, SHA256
`93ee5e5e27b9b6eea392c5c50566c53b8e0f9de28fef9d7c5c2943b7ff22d4e9`.
The runner refuses overwritten reports/work directories and changed artifacts or
original inputs. A JDK is needed only when explicitly invoking this engineering
probe; normal Rust builds and CI do not acquire these artifacts.
