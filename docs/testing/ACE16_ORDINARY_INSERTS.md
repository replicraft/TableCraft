# Ordinary scalar-row INSERT preview

The existing `NativeImage::preview_ace16_insert_rows` engineering API now builds
complete explicit ordinary rows containing supported fixed scalars and multiple
Short Text/Binary fields. The actual sole required, unique, ascending Int32
physical primary selects the key column; an earlier Int32 companion or a field's
name does not select it. Genuine logical aliases retain one physical counter
increment. No new public preview method or normal save path is introduced.

The complete focused INSERT batch passes 108 functions, including the previous
88 and six new builder, four independent record-proof and ten end-to-end
functions. The full ACE unit batch passes 403 functions with one existing ignored
test; the 108-function subset is included. Production library/example compilation
and eight example functions pass. Strict all-target/all-feature Clippy, four public
preview contracts, three compile-fail doctests, minimal compilation, strict rustdoc
and formatting pass. Four actual-source candidates now match independently frozen
complete images, followed by eight passing fresh read-only getter/map calls.
The separate public reference phase also passes four INSERTs and eight inert
getter/map calls on their first attempts. Earlier
[two-field recursive evidence](ACE16_INSERT_RECURSIVE.md) remains separately
qualified; it does not supply wider-schema or Text INSERT evidence.

## Actual ordinary-schema qualification

Two fresh Jackcess 5.0.4 public-builder originals, V2016 and V2019, each contain
FixedRows, MixedRows and Sentinel. FixedRows has thirteen fixed fields; MixedRows
has seventeen fields including two Short Text and two Binary fields. Both targets
contain 451 manually assigned Int32 keys and genuine PrimaryID/Twin aliases of
physical index zero. The primary is native column two, after a nullable Int32
companion and Boolean. Both actual Text fields store Boolean
`AllowZeroLength=true`, and no field stores `DefaultValue`. All nine bounded
source capture commands pass on their first attempts. Capture explicitly does
not establish source eligibility; four
subsequent saved-only gates check actual metadata, properties, typed raw records,
ROWs, maps and the pressure geometry before image construction.

The actual fixed extent is 69 bytes. Source FixedRows records are 71 bytes, with
3,279 bytes free on the highest hint page. MixedRows records are 97 bytes, with
only 23 bytes free on each DATA page. New explicit requests produce 71-byte fixed
or 1,106-byte mixed records. Stored Byte FF is checked against the actual signed
Java `Byte:-1` projection and Core Byte 255. DATE uses explicit UTC and
`LOCAL_DATE_TIME`; Currency, IEEE bits, GUID, masks and variable offsets are
checked independently. Existing NULL payloads and reader-valid Text wires are
decoded and preserved, rather than reconstructed with new-record defaults.

The raw Boolean declaration has length one, while the existing native reader
normalizes its physical payload width to zero. The Python oracle was corrected
to make the same distinction. No Rust writer defect or admission change was
needed. A separate source-gate receipt metadata change from 4,482 to 4,670 bytes
was detected before the producer build. The initial bundle is retained; the
87-input rebound bundle reproduces all four complete images and detail files
byte-for-byte, with source, request and model bytes unchanged.

The [committed producer build](results/ace16-insert-ordinary-example-build-2026-10-28.json)
then produces all four [matching candidates](results/ace16-insert-ordinary-native-2026-10-28.json)
on their first attempts. All four complete-image comparisons precede eight
first-attempt fresh read-only getter/map calls. Each final snapshot has 904 typed
rows across the three tables. The four cases retain all 1,804 old selected ROWs
and contain 1,808 final primary key/ROW pairs. FixedRows fits its existing DATA
page and appends two INDEX leaves; MixedRows appends one DATA page followed by
two INDEX leaves. Together the candidates add eight INDEX pages and two DATA
pages, 40,960 bytes, with no bitmap page. Each table and physical historical
counter increments once despite the logical alias.

The [maintained saved verifier](ace16_insert_ordinary/README.md) rederives all four
images before interpreting cached fresh streams. Its default mode starts no
child process; optional Git verification checks the compiled source closure.
Missing private evidence is reported explicitly rather than treated as success.

Four separate [public INSERT references](results/ace16-insert-ordinary-public-reference-2026-10-28.json)
and eight fresh inert getter/map calls also pass on their first attempts. Their
new ROWs and allocation purposes agree with the engineering cases, and the
transition proofs preserve old records and the other tables. The public proof
decodes requested values and observes index framing independently; it does not
construct the engineering images or require public scratch bytes to match them.
Complete comparisons find exactly 1,566 differing bytes per case: 783 bytes in
each fresh INDEX leaf's unused suffix. Both readers agree on the 226-entry,
prefix-zero leaf frames and all 2,034 active bytes. The retained root, DATA,
metadata, maps, counters and all other bytes agree in these four cases. The
engineering suffixes remain zero; public unused bytes do not become golden data.
These results do not establish Windows/native Microsoft Access save or reopen
behavior.

## Declared storage and explicit carriers

The source has complete current column coverage: native field numbers equal
native column positions, physical column count matches that coverage, and visible
variable ordinals form the complete unique range from zero. Historical/deleted
column gaps remain refused. Values follow native column order, while variable
payloads are assembled by their physical variable ordinal. Nonempty fixed spans
must be disjoint and their maximum end must match the source's fixed extent.
New fixed padding is zero. No schema or resource ceiling changes.

| Field | Explicit stored carrier and framing |
| --- | --- |
| Byte, Int16, Int32, Int64 | Exact corresponding integer carrier and native width |
| Currency | Exact scaled Int64 carrier; no floating conversion |
| Single, Double | Exact IEEE carrier bits, including representational distinctions |
| Legacy Date/Time | Validated legacy date carrier and its original bits |
| GUID | Exact 16-byte identity with native component byte order |
| Boolean | Boolean carrier only; value is the physical mask bit, with no payload bytes |
| Short Text | Text or NULL; positive even declared width up to 510 bytes/255 UTF-16 units |
| Short Binary | Binary or NULL; positive declared width through the existing 510-byte native ceiling |
| Decimal | Exact Decimal or NULL; qualified 17-byte variable NUMERIC, declared precision/scale retained |

Existing independent public-builder Binary fixtures declare 255 bytes; they do
not qualify every larger native declaration. Fixed fields require the supported
native type, true width, flags and zero extended flags. Text permits only the
existing Unicode-compression flag; Binary and Decimal permit no extended flags.
Date/Time Extended, Memo/OLE, complex/calculated fields and unsupported metadata
remain outside this record profile.

The primary requires a non-NULL Int32. Other nullable fixed fields serialize
NULL as zero payload with the presence bit clear. Present numeric zero retains
its presence bit. Boolean NULL is refused because its mask bit stores false or
true, not NULL. A Boolean's unused offset cannot invent fixed payload or padding;
nonzero offsets within the real fixed extent remain harmless.

Every new record stores its complete column count and physical mask. With V
variable fields, the footer has the V+1 reverse offsets, V count and mask; an
all-fixed record has no variable footer. NULL and present empty Text/Binary can
have identical empty payloads but distinct mask bits. A record must fit an
ordinary DATA page with its directory slot; no oversized-row/LVAL fallback is
introduced.

## Preferred Text bytes

New Text uses the existing variable-record preferred representation, independently
reconstructed by the audit. With compression enabled, more than two UTF-16 units
compress only when every unit is nonzero and fits one byte. Two-unit ties use raw
UTF-16. Empty Text uses zero bytes, not a compression-marker-only alternative.
NUL, wide units and surrogate pairs use raw UTF-16. Declared size counts UTF-16
units, not Unicode scalar values or UTF-8 bytes.

The existing decoder must reproduce the exact requested string. In particular,
a leading U+FEFF that aliases the compression marker is refused rather than
silently normalized. A semantically equal alternate wire does not satisfy the
request-bound preferred-byte proof.

## Empty Text declaration coverage

A request for actual empty Text requires an explicit `AllowZeroLength`
declaration in that field's first matching column property group. Absence returns
`Unsupported(ConstraintCoverage)`. This INSERT-only guard does not infer a stored
default or change shared read/update constraint diagnostics. It performs no extra
property inspection when no actual Text field requests an empty string.

Stored false satisfies declaration coverage but the existing final field check
rejects empty Text. Stored true permits empty Text independently of Required;
Required still rejects NULL. Wrong property carriers remain unsupported before
staging. NULL and nonempty Text do not trigger this additional declaration guard,
but all existing Required, validation and other mutation gates still apply.

Microsoft documents different creation defaults for
[AllowZeroLength](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/allowzerolength-property):
Yes in the Access UI and No for programmatic field creation. That statement does
not establish the meaning of an absent on-disk property. The diagnostic
`NotDeclared` status stays distinct from checked false or true; the new guard
requires evidence instead of silently choosing a default. No VBA execution or
query-function scope is added by using this storage-property reference.

## Independent record and allocation proof

The builder's working preferred serializer is shared by its existing UPDATE and
new INSERT consumers. The INSERT auditor does not call the builder profile,
fixed encoder or preferred serializer. It independently derives fixed bytes,
GUID ordering, mask semantics, variable payloads and footer geometry from the
immutable schema and actual requests. Reader decoding is a secondary structural
and value check, not construction authority. Copies, allocations, field/unit
loops and cancellation checks spend the operation budget.

The parent selects the primary from fresh original index metadata; the final
primary proof independently rebinds that column/index relation. DATA placement,
append-state transfer, recursive index normalization, map/purpose replay, exact
counter deltas and complete unowned-byte comparison retain their existing
contracts. The writer does not reuse deleted slots or rewrite existing records
to make room. Failure returns no partial candidate.

## Consumer, limits and verification scope

The working `preview_ace16_insert` clap example retains its four positional
arguments and fresh-output publisher. Its [quoted TSV protocol](ACE16_INSERT_INPUT.md)
now uses the shared exact fixed-value parsers for Boolean, integer, scaled
Currency, IEEE/date bits and GUID, alongside Text, Binary and explicit NULL.
Decimal uses the existing exact coefficient:scale parser, with no rescaling.
Complete input is captured before the database
opens, under the unchanged input and native budgets.

The first focused run retained 97 passes and one new fixture failure before the
codec: duplicate variable ordinals are already malformed in the schema parser.
The test now records that parser refusal separately from the readable unused-slot
codec refusal. No production or image policy was relaxed. The complete 108-function
batch then passes, including later-primary selection, all fixed carriers,
multiple variable ordinals, second mask bytes, preferred Text and finalizer
forgeries, explicit/absent/wrong-type empty policies, quotas and cancellation.

Defaults remain refused even with complete values or an empty batch. AutoNumber,
generated identities, extra physical indexes, wider primary/tuple/foreign profiles,
LVAL/extended-date records, required data macros and production native
saves remain separate work. Public broader-schema INSERT references provide
separate independent-writer evidence. Windows/native Microsoft Access comparisons remain
deferred; literal checks and Jackcess evidence do not establish full compatibility.

## Exact Decimal continuation

The separate engineering preview now constructs ordinary nullable Decimal fields
in their native variable slots. The carrier scale must equal the declared scale,
and the absolute coefficient must fit the declared precision. No rounding,
rescaling, Currency/float conversion or default substitution occurs. NULL owns
an empty variable span with its presence bit clear; present zero keeps the full
17-byte wire and its presence bit. Fixed/calculated/extended NUMERIC declarations
remain refused. Existing-slot update admission is unchanged.

The producer reuses the existing NUMERIC serializer. The request audit separately
derives the sign and four most-significant-first little-endian words by radix
division, then checks the complete record and final image. Eight synthetic full
images cover ACE 2016/2019, logical aliases and fitted/EOF placement, with literal
word-order and exact readback checks. Four new test functions cover complete
images, invalid declarations/carriers, independent audit reconstruction and
Decimal allocation/cancellation boundaries. All 84 INSERT unit functions and
eight example-input/publication functions pass. The example input uses the shared exact Decimal
parser and retains the fresh-output boundary.

An earlier refusal test still listed Decimal among unsupported fields; it was
updated for this explicit new capability, retaining the other refused field
families and adding positive/negative Decimal controls. No native Access or fresh
independent-writer Decimal INSERT comparison was run. Historical broader-row
receipts above retain their original scope.
