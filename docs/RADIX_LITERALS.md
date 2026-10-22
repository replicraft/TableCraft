# Hexadecimal and octal query literals

The existing owned SQL syntax for unsuffixed `&H` hexadecimal and `&O` octal
literals now evaluates in scalar and bound read queries. Values are exact integer
bit patterns; no floating conversion, locale, clock or source scan is involved.
The numeric literal implementation lives in the expression library, separate from
the parser's owned syntax and executable hosts.

| Unsigned magnitude | Current carrier and signed value |
| --- | --- |
| 0 through 32767 | Integer with the same value |
| 32768 through 65535 | Integer with magnitude minus 65536 |
| 65536 through 2147483647 | Long with the same value |
| 2147483648 through 4294967295 | Long with magnitude minus 4294967296 |
| Above unsigned 32-bit range | Overflow |

Prefixes and digits are case-insensitive. Leading zeroes do not force a wider
carrier. The prefix must be adjacent and digits unquoted; `& HFF`, `&[HFF]`, an
embedded comment or invalid digit fails syntax validation. Public host-constructed
Number nodes are also validated as complete spellings. Type suffixes and unprefixed
octal forms are not introduced by this implementation.

The same 128-byte numeric spelling ceiling applies before conversion, including
leading zeroes. This is a resource policy, not a native format limit. The helper
uses checked standard-library integer parsing and explicit signed bit interpretation;
it does not use Val's prefix-consuming grammar. Number conversion allocates no
payload; node budgets/depth and cancellation are checked by the evaluator before
this bounded work. Ordinary decimal/exponent behavior is unchanged.

Unary signs apply after radix interpretation using the existing materialized
promotion profile. Thus `&H8000` is Integer -32768, while `-&H8000` produces Long
32768 in this profile. Expression inspection, derived rows, aggregates, Eval and
crosstabs retain the actual carrier. Duplicate fixed heading names still fail even
when equivalent values use different literal spellings. Metadata-only preparation
can derive qualified fixed radix headings without scanning rows. No new index
pushdown rule is added; original filters remain authoritative.

```sh
tablecraft query eval 'SELECT &HFF, &HFFFF, &O77, VarType(&H10000)'
tablecraft query run 'SELECT ID FROM ScalarSamples WHERE ID=&H1' --database example.accdb
```

## Typing boundary and references

Plain decimal integer syntax still materializes as Int64 and fractional/exponent
syntax as Double. This patch does not infer native declared-expression types,
change decimal literal overflow behavior or admit VBA's full suffix syntax.
[MS-VBAL number tokens](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/685ad840-accb-4bdb-8bfd-f3d88498547a)
provides radix, signed-width and suffix distinctions for the explicit current
profile. It is an expression-language reference; it does not qualify native ACE/Jet
SQL literal typing or every expression context. Native Access comparison remains
pending, including width selection, leading zeroes, unary/static overflow and
result-column type inference.

The parser still preserves its owned Number spelling and never embeds a native
carrier or upstream parser AST in the query model. Binding does not execute a
provider to infer a type. No dependency, core carrier, database write, VBA runtime
or serialization format changes.

## Evidence

Three expression tests cover boundary/sign/width goldens, complete spelling,
length, budgets/cancellation and a fixed-seed generated independent integer model.
Query tests cover carriers, aggregates, Eval, subqueries, fixed named crosstabs,
no-scan preparation and colliding headings. Parser controls verify adjacency and
unquoted tokens; both CLI query commands verify buffered overflow failures.

The original [independent runner](testing/compare_radix_literals.py) passes 742
signed-width/type/native-field controls across the four unchanged original
ScalarFixture priority-family inputs. Raw getters independently supply field values;
only TableCraft executes SQL. Source bytes and the reviewed reference JAR are hash
checked. Native Access execution and typing remain unqualified.
