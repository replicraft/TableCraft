# Fresh native Increment declarations

`CreateColumn::with_incrementing_autonumber()` now declares an Int32 Increment
field explicitly. Both single-table and multiple-table creation services use
the same validation. An empty declaration may contain one through sixteen
constructible scalar fields. The generated Int32 field must come first and be
named explicitly as the primary key, with no initial rows and no field policies
or defaults. Ordinary companions retain the existing fixed, sized Short Text,
sized Short Binary and exact Decimal declaration rules. The creator writes the native flags,
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

## Initial table schemas

The empty Increment declaration now also admits the ID-only schema needed for
an initial unsaved `Table1`, ID plus Short Text, and ordinary mixed scalar
companions. The raw `canonical_initial_table_creation` fixtures independently
author these complete user definitions in the existing empty catalog/map/index
envelope. They import no production creator, decoder, native mutation or SDK
output. The mixed source includes all current fixed carrier families and sized
Text/Binary/Decimal declarations; initial rows remain empty.

This is fresh serialization support, not persistence of an unsaved GUI table or
catalog mutation in an existing database. It does not expand the separate native
Increment INSERT admission profile. Source14 observations above apply to the
original two-LONG layout; these additional schemas still require independent
reader and native Access qualification. Their focused byte/resource/cancellation
checks are tracked with the owning implementation work.

Random generation, arbitrary generated positions, initial generated rows,
reseeding and native Access save/reopen remain work.
