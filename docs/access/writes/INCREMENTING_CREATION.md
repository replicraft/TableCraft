# Fresh native Increment declarations

`CreateColumn::with_incrementing_autonumber()` now declares an Int32 Increment
field explicitly. Both single-table and multiple-table creation services use
the same validation. The initial supported declaration has two Int32 fields,
generated first, the same field named explicitly as the primary key, no initial
rows and no field policies or defaults. The creator writes the native flags,
LAST zero and an empty primary leaf; it does not generate initial IDs or infer
a seed from supplied rows.

```text
tablecraft create fresh.accdb --table Items --column Id --column Score --primary-key Id PrimaryId --incrementing Id
tablecraft query journal-insert "INSERT INTO Items (Score) VALUES (17)" --database fresh.accdb --journal fresh-write
```

Typed `--field` declarations accept the same explicit selector. Unsupported,
missing, mismatched or repeated selectors and nonempty initial IDs/NULL refuse
before publication. Ordinary creation keeps its supplied-value semantics.
Later [normal INSERT](NORMAL_INCREMENTING_AUTONUMBER.md) reopens and qualifies
the actual native bytes; the earlier builder choice is not mutation authority.

All fourteen independent source views were loaded and hashed before matching
creator calls. Four builder/order whole-image comparisons, exact resource and
cancellation controls, typed/SQL create-then-insert saves and both CLI schema
modes pass. The two-table test checks independently authored page components;
it is not a separately SDK-qualified complete multi-table image. Forty-five
existing native creation tests and sixteen CLI creation tests pass. See the
[execution record](../../testing/results/ace16-increment-creation-2026-11-01.json).

Random generation, arbitrary generated layouts, initial generated rows,
reseeding and native Access save/reopen remain work.
