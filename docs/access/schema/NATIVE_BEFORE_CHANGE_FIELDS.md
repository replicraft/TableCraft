# Explicit BeforeChange field preparation

`NativeImage::prepare_ace16_before_change_fields` prepares caller-selected
BeforeChange programs against an immutable ACE16 image and existing exact INSERT
or single-column fixed UPDATE intents. It returns logical final rows and a
pass/rejection trace. It performs no file write, creates no physical candidate and
provides no normal-save permission. Native storage discovery, required-event
coverage and Microsoft Access save/reopen qualification remain unfinished.

The feature requires `ace16-write-preview`. Every ordinary source field must be
Int32/NULL with the existing qualified fixed flags. INSERT supplies every field;
defaults, generated fields, cascades, other-table actions and ambient callbacks
are unsupported. The inherited source profile also excludes a field named
IsInsert. BeforeDelete refuses this profile.

## Source and execution

Explicit selectors bind exact original Extra group-kind/property-name pairs.
Every parsed property must be selected and compiled, even for no selected rows or
unreachable branches. Opaque groups, unrepresented names, nontext/unknown bodies,
malformed programs and other event contexts refuse. The source table's body,
module and child payload checks remain in place. No locator or native dispatch
order is inferred from these selectors or from a successful result.

The additive `MacroProgramProfile::BeforeChangeFields` recognizes the documented
BeforeChange `SetField` shape with exactly one `Field` and one `Value` argument.
`Field` binds directly to an actual metadata name; it is not parsed as SQL. Value
uses the existing closed expression subset. A successful assignment updates
Current for subsequent statements and conditions; Old stays frozen at the actual
original row. Each row has its own Current frame. `Updated` refuses this new
profile because its timing after SetField remains unqualified.

NULL remains NULL. Integral Byte/Int16/Int32/Int64 results use an explicit checked
signed-Long target conversion; Boolean, text and other carriers refuse. Conditions
remain Boolean-only. RaiseError returns a completed rejection and discards every
final row, including earlier rows/programs. Resource, cancellation and evaluation
errors return no completed preparation.

`final_rows()` returns all fields in native order on success, including
`Some(empty)` for an empty operation; rejection returns None. Ordinals remain
original-image/request local. Assignment locations and the control-flow trace
contain no expressions or row values. Source bytes and exact selected property
witnesses remain available separately. A fresh rebind reconstructs results from
original source, selectors and requests before exposing the preparation.

## Desktop contract and limits

Microsoft's [Before Change documentation](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/before-change-macro-event)
shows direct SetField with `Field`/`Value` XML arguments and describes Old values
and rejection of event changes. The generic
[SetField article](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/setfield-macro-action)
describes CreateRecord/EditRecord context and labels the designer target Name.
This implementation selects the specific BeforeChange profile; it does not admit
those general record blocks or invent a `Name` XML alias.

Sequential Current with frozen Old is an explicit engineering preparation law,
not an observed native timing result. The documented
[RaiseError logging behavior](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/raiseerror-macro-action)
is not executed. Other native events, named calls, handlers and transaction
side effects remain outside this profile. The existing
[guard-only service](NATIVE_BEFORE_EVENT_GUARDS.md) and its structural profile keep
their previous behavior. All [normal ACE write gates](../../testing/DATA_MACROS.md)
remain unchanged.

## Observed checks

SOURCE passed before evaluation for 18 independent programs across both ACE16
headers: 36 complete native/MR2/XML carriers. Four API functions passed on their
first execution: all 36 literal outcomes/final rows, original-intent refusals,
complete Extra coverage for zero-row operations, exact resource pools/one-below
and final cancellation. Two causal controls reject correlated instruction/final
row changes, a forged frozen Old frame and a changed program witness.

Six guard-authority and 46 Extra functions pass. After the static metadata-name
correction, all four logical API and two causal functions pass again, together
with ACE library strict/all-feature and formatter checks. The minimal build
passed; the subsequent target-binding correction is preview-feature-only.
Existing CLI compilation/strict checks, 65 unit functions and 22 copy/inspection
integration functions also pass with the additive structural enum. No new CLI
SetField command is introduced. The [focused result](../../testing/results/ace16-before-change-fields-logical-2026-10-30.json)
retains those exact phases. Physical source images and an atomic engineering
consumer are a separate tranche, with no physical writer claim here.
