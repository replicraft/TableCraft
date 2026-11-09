# Exact legacy Date defaults

Fresh creation accepts typed legacy Date defaults whose explicit ISO civil
literal preserves every original DATE bit. The existing omitted-field INSERT
service resolves these literals from original metadata and preserves supplied
NULL rather than replacing it with the default. Native intents retain original
field presence; recovery rederives omitted values through the same qualified
source and fresh independent proof.

The creator uses the existing Jiff-backed calendar conversion and query literal
parser, verifies bit-exact calendar replay, and writes a bounded four-digit
ISO date/time expression with optional fractional seconds. No locale, current
year, clock, floating-point text formatting or executable expression is inferred.
The insertion parser accepts that explicit grammar and exact native Date column
metadata. The proof independently parses fixed civil components and constructs
the DATE value without calling the producer's parser.

Six independently modeled cases cover the epoch, a negative whole day with a
positive time fraction, leap-day noon, year 100, year 9999 and fractional seconds.
Both source header variants have before/omitted/supplied-NULL images. Creation
comparisons use the creator's actual header profile; they do not claim creation
of a second header profile. Four representational aliases—negative zero,
negative epoch fraction, subnanosecond and rounded midnight—refuse before
publication rather than silently canonicalizing to a different DATE bit pattern.

These checks qualify local engine and retained-session behavior. Microsoft
Access save/reopen remains unrun; generated and clock defaults keep their
separate contracts.

The [execution record](../../../tests/results/schema/ace16-date-default-writes-2026-11-01.json)
separates creator, native proof/recovery and retained-session checks.
