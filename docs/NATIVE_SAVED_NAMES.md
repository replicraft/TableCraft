# Stored saved-query identifier spellings

Native local saved SELECT/crosstab reconstruction accepts literal names or names
already wrapped in one Access bracket pair. `[Order Items]` becomes the owned name
`Order Items`, not an object whose literal name includes brackets. The original
native fragment remains unchanged. This applies to table names, source aliases,
projection/TRANSFORM aliases and the literal parameter-name profile.
[Literal parameter prompts](PARAMETER_PROMPTS.md) now have a separate punctuated
name grammar; the object-name rules below continue to apply to objects/aliases.

The reviewed Jackcess 5.0.4
[QueryImpl identifier renderer](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/query/QueryImpl.java)
preserves an existing outer bracket pair instead of quoting it again. Microsoft's
[object naming guidance](https://support.microsoft.com/en-us/access/guidelines-for-naming-fields-controls-and-objects)
excludes period, bang, grave accent and brackets from object names; its
[reserved-name guidance](https://support.microsoft.com/en-us/access/learn-about-access-reserved-words-and-symbols)
explains bracketed references to reserved words. These distinguish delimiter
syntax from name content, not a qualification of TableCraft's full naming rules.

## Adaptation and identity

The adapter borrows the body of one complete outer bracket pair. Empty bodies,
remaining/mismatched/nested brackets, controls and dot/bang/grave-accent name
content fail. It does not unescape arbitrary SQL, split a qualified path, trim
spaces, change case or repeatedly remove delimiters. Legal punctuation remains
inside a single bracketed identifier while the bounded parser constructs owned
names; semicolons/quotes/operators cannot become clauses through this path.

Logical names drive binding and duplicate detection. `P` and `[p]` are duplicate
ASCII parameters; aliases `a` and `[A]` are likewise ambiguous, even if their raw
metadata differ. Native join endpoints separately match the original stored
labels, ASCII case-insensitively, including delimiters. This follows the reference
renderer rather than inventing an implicit source or repairing an endpoint that
does not match a declaration. A source alias `[a]` therefore needs a matching
stored endpoint `[a]`, not an undeclared endpoint `a`. Owned SQL still uses alias
`a`. Anonymous derived sources retain no endpoint label.

The helper allocates nothing and lives in the Access syntax adapter. Existing
name/source text work, fallible reservations, parser byte/token/depth allowances
and final cancellation checks remain. It introduces no foundational name-policy
change, query-model constructor, dependency or write capability. Inert printing
still neither scans user rows nor resolves calls; explicit execution uses the
same binder, parameter scopes, snapshots and buffered publication.

## Limits and evidence

Qualified stored object names and Forms/control references remain gaps. Literal
parameter punctuation is now supported under its separate prompt/reference rules. Parameter matching
and multi-source endpoint identity use ASCII-insensitive, otherwise exact matching; this does not provide native
Unicode/NLS collation or authoring validation for all Access object types.

Five feature tests cover expected owned syntax and unchanged raw fragments,
join labels and normalized duplicates, malformed names/injection, native parameter
execution over four header layouts and generated symbol-bearing aliases. One
crosstab test checks source/row/TRANSFORM aliases. One CLI test checks saved/named
retrieval, literal headers, prompt binding and missing-value buffering. Existing
resource/cancellation tests continue to pass.

The original [SavedIdentifiersFixture.java](testing/SavedIdentifiersFixture.java)
adds ten inert definitions to our fourteen-query forest recipe. Four fresh local
Jet 4 / ACE 2007/2016/2019 files compare through independent SQL rendering and
explicit expected rows using [compare_saved_identifiers.py](testing/compare_saved_identifiers.py).
All 160 direct/saved/named reconstruction/execution controls pass with source bytes
unchanged. Symbols, reserved names, prequoted tables/aliases/columns/prompts,
quoted join endpoints, a derived alias and a single Unicode alias are covered.
[Fixture records](FIXTURES.md#stored-identifier-inputs) retain provenance and hashes.
The oracle renders metadata but never evaluates SQL. Native Microsoft Access
execution and application save/reopen remain unrun.

```sh
python3 docs/testing/compare_saved_identifiers.py \
  --jar path/to/jackcess-5.0.4.jar --cli target/release/tablecraft \
  --directory path/to/fresh-stored-names --generate
```
