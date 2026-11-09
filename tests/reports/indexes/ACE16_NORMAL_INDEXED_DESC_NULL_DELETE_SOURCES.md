# ACE16 descending and stored-NULL DELETE sources

Two new inert Jackcess 5.0.4 originals qualify the existing normal DELETE grammar
for descending nullable Int32 keys. Items has three manual LONG fields and rows
`(1,-1,NULL)`, `(2,0,1)`, `(3,1,2)`. PrimaryID/PrimaryAlias share an ascending
required-unique physical index; GroupOrder/GroupAlias share a descending ordinary
index that stores NULL. Actual cursor order is Guard 2,1,NULL. No AutoNumber,
default, macro, form, expression or link was authored.

The original generator uses exclusive CREATE_NEW through the reviewed bounded
channel, expression evaluation disabled and an external resolver that refuses.
Its explicit `IndexBuilder.addColumns(false,"Guard")` uses the pinned primary
SDK direction API. Generated originals, all aliases/counters, raw fields/masks,
physical PAGE/TD/DATA/index bytes and twelve original read observations were
frozen before authoring DELETE AFTER laws. Original creation is not a native
Access comparison or a permission to edit arbitrary files.

The fixed source model removes the NULL row, a present middle row, both exterior
rows or all rows across both header 5/header 6 originals. Both exterior request
orders share one independently encoded image. Guard present entries are 9B;
NULL entries are 5B including the original ROW suffix. Nullable presence remains
separate from the inactive numeric payload. The model changes only selected DATA
root tombstones, TD live count and both leaf free/bitmap/survivor-prefix bytes.
Histories 3/3, four logical aliases, maps/EOF, all original payloads/masks and every
vacated/inactive leaf byte remain exact. Empty roots remain allocated leaves.

Observed results:

- Pure construction passed first attempt for eight AFTERs/sixteen read views.
- Eighty exact independent observations passed first attempt: sixteen complete
  inventories and sixty-four alias cursors. Saved-only verification confirms
  argv/identity/exit/empty stderr and all frozen source/code/class/JAR pins.
- Native source gate passed one test covering all sixteen read/constraint/index/
  Source views. Its first Cargo attempt failed before native execution because
  the draft referenced undeclared serde_json. A retained compile-only successor
  uses literal file/length/SHA tuples and a whole-manifest hash; no dependency,
  request law, expected bytes or source image changed.
- Matching native producer/recovery/forgery acceptance then passed all three
  functions without a production mutation change. The separately maintained
  [indexed write follow-on record](../../results/indexes/ace16-indexed-write-followons-2026-11-01.json)
  owns its exact commit/recovery/forgery and strict-check outcomes.

The source-phase zero-producer counts are historical. Later native write checks
are separately identified; SDK source reads are not producer-derived expectations.
The follow-on record also owns the passed broader checks; this source record
does not duplicate them. Native Access save/reopen
and Windows comparisons remain unrun. No node contraction, retirement or wider
key-family compatibility is inferred.

The maintained source bundle contains the original-only Java recipe, pure model,
independent full-report predictor, literal request laws and a create_expected
replay tool. The replay tool accepts the two exact pinned originals and complete
original reports plus a fresh output directory. It invokes no SDK, reader,
network, native writer or subprocess. New SDK generation can vary bootstrap
bytes and needs separate source qualification; the fixed model never grants
creator/hash-based permission to normal writes. The maintained replay now reproduces all 104 historical images, proofs and
reader predictions byte-for-byte, with no new SDK or native calls.
