# Native aggregate value and carrier observations

The original MIT [portable recipe](native_aggregate_oracle.py) prepares 100 fixed
SQL probes for Count, Sum, Avg, Min, Max, First and Last. Seven explicit numeric
casts cover Byte, Integer, Long, Single, Double, Currency and Decimal, each over
populated and empty inputs. Separate Count(*) and Count(V) probes include a NULL
row. These are executable probes, not recorded native results.

The [Windows collector](Capture-NativeAggregates.ps1) requires a matching-bitness
native ACE DAO installation. It creates one fresh database containing the four
original values 1, 2, 3 and NULL. It never opens an existing application database.
A fixed request digest rejects edited SQL, and existing preparation/capture
folders are refused rather than overwritten. Reads reopen the created file
read-only, and a before/after digest checks unchanged database bytes.

For each successful query, the collector records the numeric value, VarType
result, DAO field type and size. Single/Double use exact bit encodings, including
signed zero. Currency/Decimal retain the four CLR Decimal words, preserving
coefficient, scale and sign. Query failures retain HRESULT/DAO numbers without
partial fields or rows. Unexpected collector failures abort capture. Engine DLL,
interface, Windows and PowerShell versions plus process bitness accompany the
request, runner and database hashes and the original header prefix.

## Portable preparation and exercise

```sh
python3 docs/testing/native_aggregate_oracle.py prepare \
  .cache/tablecraft-fixtures/native-aggregate-probes
python3 docs/testing/native_aggregate_oracle.py exercise \
  .cache/tablecraft-fixtures/native-aggregate-probes --cli target/debug/tablecraft
```

Exercise runs the same 100 query shapes through TableCraft with equivalent inline
Long/NULL inputs. Independent integer expectations check the values of counts,
sums, means and selections. Runtime subtype output must be present, but no native
subtype is invented. All 100 controls pass on the current CLI. This is TableCraft
execution evidence only; it neither creates a native database nor qualifies ACE.

## Explicit native capture and comparison

Copy the prepared request directory to Windows, using the reviewed collector:

```powershell
./docs/testing/Capture-NativeAggregates.ps1 -Directory ./native-aggregate-probes
```

Retain the complete directory, then validate and compare it with a TableCraft CLI:

```sh
python3 docs/testing/native_aggregate_oracle.py validate ./native-aggregate-probes
python3 docs/testing/native_aggregate_oracle.py compare ./native-aggregate-probes \
  --cli target/debug/tablecraft
```

Validation requires the original request/runner, complete ordered case coverage,
lossless cells consistent with their field carriers and unchanged database bytes.
It separates native errors from captured results. Hash checks establish artifact
consistency, not authenticity or compatibility. Synthetic reports in portable
unit tests are explicitly not native evidence.

Comparison reads only the captured database with the fixed recipe. It separately
reports numeric-value matches and runtime VarType matches; native DAO field types
remain recorded evidence, not an inferred mapping from TableCraft metadata.
Errors, timeouts, malformed CLI output and mismatches never count as passes.
The original database hash is checked after comparison. Native errors keep a case
unqualified even if TableCraft also refuses it. The command succeeds only when all
100 native cases have matching values and runtime subtypes.

## Current limits and reference review

Nine portable integrity/comparison tests pass, including tampering, partial
results, numeric encodings, signed zero, carrier mismatches, failed processes,
timeouts and mutation during comparison. The existing twelve native-text oracle
tests also pass. Windows/PowerShell/ACE execution has not run in this environment;
no native capture or passing native comparison exists for this recipe. An ACE 16
claim requires the recorded DLL/build and actual native results, regardless of
the installed interface's version string or created file's generation.

Microsoft's [Count query contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/count-function-microsoft-access-sql)
documents NULL omission and the wildcard exception. The
[DCount declaration](https://learn.microsoft.com/en-us/office/vba/api/access.application.dcount)
returns Variant without establishing the narrower numeric carrier in that page.
These references do not justify changing the current explicit Int64 Count/DCount
profile to a guessed native subtype. Collection Count/RecordCount properties are
separate APIs. DCount needs separate Access-host observations and is not qualified
by this DAO SQL collector. Native result promotion, error precedence and complete
P0 value semantics remain acceptance work.
