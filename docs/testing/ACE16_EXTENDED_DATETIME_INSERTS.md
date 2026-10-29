# Extended Date/Time ordinary INSERT preview

The separate `NativeImage::preview_ace16_insert_rows` engineering API now accepts
ordinary Extended Date/Time payloads under the existing ACE2019 read profile.
The source column must be native type `0x14`, fixed-width 42 bytes, with the
qualified ordinary fixed flags and no extended flags or complex identity.
ACE2016 declarations remain refused. This does not add Extended Date/Time index
keys, normal backend writes or native save qualification.

## Exact carrier and record bytes

Present values require `Value::DateTimeExtended`, whose Core constructor validates
Gregorian years 1 through 9999, civil time and a fraction of 0 through 9,999,999
units of 100 ns. The writer does not accept Text, legacy Date/Time, floating bits
or another numeric carrier in its place. No timezone, locale conversion,
normalization or fractional rounding is involved.

The existing native decoder and new encoder own this complete 42-byte grammar:

```text
19 ASCII day-offset digits : 12 ASCII second-of-day digits 7 fraction digits :7 NUL
```

Day zero is 0001-01-01; the maximum day is 3,652,058. Seconds range from zero
through 86,399. The encoder uses checked civil-day arithmetic and writes directly
into a fixed-size array. The existing INSERT record layout accounts for the full
fixed span and rejects overlaps or a mismatched fixed extent.

A new NULL has 42 zero payload bytes and a clear presence bit. A present epoch
instead has the canonical ASCII representation and a set presence bit. Existing
NULL payload bytes remain opaque and preserved, even when they would not decode
as a present timestamp. Complete-row masks, other fields, index membership,
counter deltas and unowned bytes retain their existing independent checks.

## Engineering input

The [quoted TSV example](ACE16_INSERT_INPUT.md) has an explicit `extended-date`
kind with exactly 27 ASCII DATA bytes:

```text
extended-date	2000-02-29T12:34:56.1234567
```

The syntax is exactly `YYYY-MM-DDTHH:MM:SS.fffffff`: uppercase `T`, seven
fraction digits and no timezone suffix. The boundary parser validates fixed
digits/separators and calls the Core constructor directly. Shorter or longer
fractions, invalid civil dates, leap seconds and offsets refuse rather than
rounding or applying library parsing defaults. Text input remains Text; NULL
still uses its own empty `null` pair. Existing shared parsers for other engineering
examples are unchanged.

Input is fully validated before opening the database. The backend then checks
the actual source profile, declaration, carrier and all existing write blockers.
Input acceptance alone grants no source eligibility. The default 1,024-byte
value limit and all file/native budgets remain unchanged.

## Independent evidence and current limits

The first read-only gate validates both complete source and expected images
before producer admission changes or candidate execution. The literal ACE2019
fixture has three fixed fields, a 52-byte fixed extent and a one-byte mask, with
two physical Int32 indexes and their logical aliases. It retains an old opaque
NULL payload and independently adds leap-day, one-tick, NULL and maximum values.
Complete candidate bytes and raw key/ROW history effects are checked;
the date field is not indexed.

Four public functions cover whole-image preservation, exact values, wrong
carriers, the earlier-profile refusal, all resource pools and cancellation at the
measured terminal healthy callback. Separate codec and independent record-audit
controls use literal boundaries and independent Gregorian arithmetic, including
the non-leap year 1900 and leap year 2000. The first focused batch passes 15
functions, the first combined INSERT batch passes 258, and the engineering
example passes 16 functions including three new strict-input controls. The full
ACE library passes 601 functions with one existing ignored generator. Full Access
integration passes 500 after a separate required-index read-order correction.
Strict ACE Clippy across all targets/features, minimal ACE/Access compilation,
formatting and documentation/dependency guards pass. These totals overlap and
are not summed.

No fresh public-builder original, Microsoft Access comparison or native
save/reopen evidence is claimed for this new payload continuation. Required
data macros, defaults/generated fields and every other existing write blocker
remain in force. See the [ordinary-row contract](ACE16_ORDINARY_INSERTS.md) for
the surrounding profile.
