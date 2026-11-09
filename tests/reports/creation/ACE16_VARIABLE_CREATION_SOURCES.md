# Variable-scalar creation sources

Five independent header-5 sources describe an ordinary `Items` table with
Int32 Id, Short Text, Short Binary, NUMERIC and Boolean fields. They reuse the
qualified canonical nine-page envelope and independently replace only the user
definition and DATA pages. No production constructor or encoder supplies their
bytes. Native engine checks passed two tests over thirteen total rows; the six
exported originals then passed complete-image and full SDK-observation checks.
This is a source phase, not an implemented variable creation API.

Text declares eight UTF-16 units (native width 16 bytes), Binary sixteen bytes,
and NUMERIC precision 28/scale 4 with seventeen-byte storage. The three variable
slots follow the actual descriptor order. Each row has a six-byte fixed prefix,
EOD and three reversed start offsets, a variable count and a presence/value mask.
NULL differs from present empty Text/Binary even when their spans are identical.
Present Decimal zero retains its full payload and scale; Boolean has no payload
or NULL carrier.

The five cases cover zero rows; NULL/empty combinations; non-BMP Unicode,
embedded NUL and eight-unit text; binary boundaries; Decimal word order, signed
precision endpoints and retained-scale zero; and leading U+FEFF. The latter uses
an explicit marker/mode-switch representation, including a nineteen-byte payload
for eight decoded units. A separate reader control contains only raw `FF FE`
and expects empty Text, rather than treating that alias as a positive creation
expectation. There are twelve positive rows plus one alias-control row.

The source envelope is recorded in the canonical V2 source result (fixture
commit `6111e6d2`). Fixed and IEEE creation sources (`fcadc73d`, `d8c7509c`)
remain unchanged. This phase does not establish variable producer output,
native Access/Windows behavior, imported-file save permission, validation-rule
or macro eligibility. Required/AllowZeroLength metadata is not authored here.

The first native test compilation failed on a missing Cursor import before any
test ran; the import correction changed no source bytes. The first SDK helper
read the empty source successfully, then failed its own raw-footer validation
on null-empty; four sources were not called in that attempt. Its corrected
footer-index version read all six originals successfully, and the saved verifier
matched every complete image and observation. These attempts are retained
separately; no original or positive expectation was retuned. The ordinary CI
hash anchor passed for all six originals. Final strict and format checks passed.

The [source result](../../results/creation/ace16-variable-creation-source-2026-10-31.json) records exact inputs, observations and retained
failed attempts separately from production creation and native Access qualification.
