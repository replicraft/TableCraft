# Partition range labels

`Partition(number,start,stop,interval)` now runs in scalar evaluation and bound
read-only queries. Microsoft's [Access reference](https://support.microsoft.com/en-us/access/partition-function)
includes a grouped query example. Its
[VBA language reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/partition-function)
also specifies numeric rounding and NULL handling. These are query-function
contracts, not a requirement to execute stored VBA.

The numeric profile uses checked Long conversion with half-even rounding. Start
must be nonnegative, stop greater than start, and interval at least one. Buckets
begin at start and the final bucket ends at stop. Outside values produce an empty
lower or upper side. Interval one follows the documented `number:number` rule,
including outside values after validating the range settings. Any explicit NULL
returns NULL after operand evaluation. [Legacy DATE serials](DATE_NUMERIC.md) now use the same Long conversion;
SQL numeric Text uses the explicit policy below; Extended-date/object coercion
remains unsupported.

## Output and boundaries

Output is ASCII Text with a colon separator. Each side is right-aligned to the
greater decimal width of stop+1 and start-1; empty sides contain spaces. A number
wider than that minimum is retained. This follows the endpoint examples and
the [related .NET padding contract](https://learn.microsoft.com/en-us/dotnet/api/microsoft.visualbasic.interaction.partition?view=net-5.0).
The Access prose is ambiguous about padding width; exact native padding and the
interval-one/outside-range interaction remain unqualified.

```sh
tablecraft query eval 'SELECT Partition(1010,100,1010,20) AS Bucket'
tablecraft query eval 'SELECT Partition(100,0,99,5) AS AfterLast'
```

The Text values are `1000:1010` and `100:   `, respectively. CLI Text display adds
quotes and the CSV crate handles TSV transport; neither changes label spaces.

Long input bounds are explicit. Intermediate arithmetic widens to i64 so neither
stop+1 nor bucket arithmetic overflows the accepted domain. Numeric conversion
overflow and non-finite input retain existing typed failures. Invalid range
settings return Domain. A NULL argument suppresses conversion/domain validation,
but never skips evaluation of another supplied expression. Wrong arity, missing
slots, wildcards and qualified names fail before executing operands.

The expression crate owns a stack-only recipe under `functions::text::partition`.
Its bounded formatter emits through TextOutput; the host checks cancellation and
charges output bytes before allocation/publication. Stored values and schemas do
not gain a new carrier. Queries retain the ordinary Text-call dependency walks,
grouping identity and metering. Label comparisons/grouping/sorting require the
existing explicit collation policy. Native metadata inspection remains inert.

## Evidence and limits

Four kernel tests include independent boundary/space goldens, half-even conversion,
NULL/domain/carrier checks, work/output/cancellation limits and a fixed-seed
256-case enumerated-bucket property. Query tests cover grouping, filter/correlation
dependencies, aggregates, plan reuse, eager errors, syntax refusal and every
observed cancellation checkpoint. CLI tests check TSV-decoded spaces, NULL and
buffered failures in both execution paths.

The original [independent runner](testing/compare_partition.py) enumerates small
buckets, rounds with Python Decimal and checks unchanged original
SavedForest-derived native files. It adds no saved Partition query fixtures and
does not execute Microsoft Access expressions. Complete Variant conversion,
native width/rounding/error precedence and padding remain unqualified. Partition
stays **Partial** in the [built-in tracker](ACCESS_BUILTINS.md).


## Explicit numeric Text controls (23 October 2026)

The SQL evaluator now accepts numeric Text in any of Partition's four positions
under a supplied NumberFormat. The shared kernel retains its NULL precheck and
converts non-NULL controls to Long in written order before validating range
settings. Thus NULL suppresses conversion of malformed Text, while an invalid
range does not hide an earlier conversion failure. All supplied SQL expressions
still evaluate eagerly, including division errors beside NULL. Input Text limits
and cancellation/work checks remain applicable before successful publication.

One internal converter supplies existing checked numeric parsing; the kernel's
public host-free path retains its original numeric-only boundary. Binding needs
no format provider and reusable parameter plans consult the current execution
policy. Labels, padding, Long bounds, interval-one behavior and the formal Variant
return category remain unchanged. The existing primary references establish
nearest-even rounding and NULL behavior; native Text coercion, padding and error
precedence still require Access comparisons.

The original [range runner](testing/compare_partition.py) now also checks all-Text
controls under en-US/de-DE, malformed/overflowing controls, NULL/eager failures and
four registered large-index fixture families. It independently enumerates buckets
and rounds with Python Decimal; unchanged source hashes remain checked.


## Ordered expression-state admission

Partition now preserves eager Error/Empty/Missing inputs through its NULL
precheck and all four Long conversions. NULL suppresses conversion failures,
including Error data; operational failures while evaluating any operand still
propagate. All controls convert before range validation. Error data needs an
explicit conversion; Missing is not a scalar. Host Empty selects numeric zero,
without adding a SQL Empty constructor. Shared preparation retains labels and
padding, while normalization and output use the same cumulative byte budget.

Focused tests cover exact output/normalization allowances, every observed
successful cancellation checkpoint, derived Error values and once-only volatile
operands. The [independent state runner](testing/compare_partition_hyperlink_states.py)
checks NULL/error ordering and labels against a separately enumerated model over
scalar calls and unchanged registered native inputs. Only TableCraft executes
expressions; native Error/Empty precedence remains unqualified.
