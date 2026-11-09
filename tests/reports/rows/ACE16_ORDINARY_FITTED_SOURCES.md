# Ordinary fitting Text/Binary sources

The independent `canonical_ordinary_fitted` testkit owner authors an eight-field
Items table in the existing canonical nine-page envelope. It imports no native
creator, serializer, planner or decoder. Catalog LvProp remains NULL with an
empty physical span; no properties are stripped from a genuine or rich source.
Existing rich fitted fixtures have nonempty property bags and are references for
record geometry, not silently admitted originals.

Fields are Id, TextValue, BinaryValue, DecimalValue, Flag, Large, Money and Date.
Text declares eight UTF-16 units and Unicode compression; Binary declares sixteen
bytes. Decimal p28/s4 remains unchanged. The fixed prefix is 30 bytes, with three
variable slots and an eleven-byte footer. Original record lengths are 80/53/43,
with slack 8/12/0, starts 30,40,44,61 / 30,30,30,30 / 30,32,32,32 and masks
ef/11/e7. NULL fixed payloads and a5 slack provide preservation witnesses.

Nineteen requests per header5/6 supply 38 healthy BEFORE views and 30 literal
AFTER views. Positives cover NULL versus empty, compressed/raw Unicode and escaped
BOM, Binary growth, shared exact capacity in both group orders, all five optional
fixed families, disjoint rows, exact/NULL noops and empty groups. Four requests per
header have no AFTER: two capacity excesses and stale Text/Binary expectations.
AFTER bytes come only from original records and literal requests. Shared Text
10→6 plus Binary4→16 uses exactly eight slack bytes; siblings, remaining slack,
directories, maps, catalog and whole-file complement remain exact.

The first private draft declared compression flag0 while expecting compressed
beta. Static review caught this before execution; the byte-exact predecessor is
retained and the successor explicitly declares flag1. Escaped BOM remains a valid
positive. No producer observation was used to revise expected images.

The source gate passes one test over all 68 views, checking native schema, rows,
constraints, absent indexes, raw framing/complement and the existing read-only
ordinary fixed-table admission check. Its first compile failed on a duplicate
import. The next read failed because existing content admission required active
EOD to equal the footer; the corrected guard permits protected inactive slack
when EOD is bounded by the actual footer. Literal source/AFTER bytes did not change.
The third gate passes. See the [source result](../../results/rows/ace16-ordinary-fitted-sources-2026-10-31.json).

No SDK or native Microsoft Access comparison has run for this source set. These
are authored-schema source results. Normal write acceptance is recorded separately.
