# Retained String comparison

Relational comparisons can now pass complete raw UTF-16 units to an explicitly
selected provider without first publishing Unicode Text. This includes isolated
surrogates, mixed decoded/raw operands and complete units beginning at odd byte
offsets. Odd byte counts remain UnrepresentableText. No replacement, padding,
truncation or implicit binary ordering is introduced.

`Collation::string_comparison_profile` defaults to Unicode. Providers must
explicitly select `StringComparisonProfile::Utf16` and implement `compare_utf16`
to receive raw units. Its SQL comparison policy is separate from
`compare_like_units`: opting into one does not opt into the other. Provider errors
propagate without retries, decoding fallback or a different locale/provider.

The bounded `comparison::compare_utf16_strings` service accepts borrowed
`StringInput::Text` / `StringInput::Utf16Le` operands. The existing LikeString
name is retained as an alias for the same input representation. Unit conversion
is shared with the matcher, with fallible allocation, explicit little-endian
decoding and cumulative work/cancellation guards. Raw lengths retain the Long
byte ceiling; decoded operands retain the evaluator's per-Text UTF-8 ceiling.
Raw buffers have no fabricated UTF-8 size. Providers separately bound their
callback allocations/work and must not infer a default locale.

The expression evaluator selects this route only for String/String comparisons
containing raw data. NULL and Error/state admission retain their existing
contracts. Mixed numeric/date coercion still uses strict Unicode publication;
the explicit both-known-Variant String/numeric category exception remains
independent of String contents. Scalar-only providers also retain strict
publication. Existing ordinary decoded comparison semantics are unchanged.

Query relational operators, lists/ranges, membership and ANY/ALL use the same
boundary. Static and retained runtime field policies select the comparator;
required unknown or conflicting policies fail without fake Text placeholders.
Both operands still evaluate eagerly. Qualified projections, WHERE and joins can
compare raw Strings while returning ordinary scalar results.

The standalone engine's `compare_general_utf16` supplies the existing provisional
General legacy/2010 model over complete units. It shares the SQL trailing-ASCII-
space rule with `compare_general_text`; LIKE keeps literal spaces. GeneralCollation
and the explicit CLI General policies forward this separate contract:

```sh
tablecraft query run "SELECT (ChrW(55296)&'A ')=(ChrW(55296)&'a') AS Matches" --collation general-2010
```

This returns TRUE under the explicit model. General legacy/2010 retain their
different recorded surrogate weights; neither silently becomes binary order.
No additional native locale, mixed-policy precedence rule or index pushdown was
added. The independent key vectors establish the provisional model, not native
Microsoft Access SQL conformance.

[Raw DISTINCT/UNION/group/sort keys](../../query/keys/RAW_STRING_KEYS.md) now use this comparison
boundary when an actual comparison is needed. [Min/Max and domain extrema](../../query/aggregates/EXPRESSION_AGGREGATES.md#retained-string-extrema)
also retain Strings under these strict key rules. [Database-mode StrComp](STRING_COMPARISON.md#lossless-database-string-operands)
now shares the explicit raw-unit provider while keeping its default-policy routing
separate from field-policy SQL predicates. Other consumers and wider raw
coercion remain separate acceptance work. Final query/CLI String
publication remains strict: directly selecting an unrepresentable String still
fails before buffered results are returned. Stored values, source/parameter
admission, write gates and physical index encodings are unchanged.

Verification includes generated unsigned-unit comparison against an independent
model, every observed pure/query cancellation checkpoint, zero byte/work budgets,
odd/unaligned input, provider refusal without retry, field policy routing,
General SQL/LIKE space differences and relational consumers. The original
[runner](../../testing/compare_raw_string_order.py) passes 17,345 controls against the
registered independent short key vectors, including isolated units. Existing raw
LIKE/predicate, retained-cell and native-policy runners also pass. Full workspace
tests, benchmarks and Windows Access qualification were not run for this batch.
