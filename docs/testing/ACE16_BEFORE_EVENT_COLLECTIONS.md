# Complete explicitly selected before-event collections

The existing explicit [BeforeChange field preparation](../access/schema/NATIVE_BEFORE_CHANGE_FIELDS.md)
and [fixed UPDATE candidate](ACE16_BEFORE_CHANGE_FIELD_COPIES.md) now accept a
complete selected Extra collection containing both BeforeChange and BeforeDelete
programs. This introduces no automatic event discovery, normal ACE save permission
or physical INSERT/DELETE consumer.

## Preflight everything, dispatch the matching event

Every selected property and envelope compiles before any row executes, including
inactive events and false branches. Missing or duplicate selectors, incomplete
Extra coverage, opaque structures, unsupported events/actions and invalid dormant
expressions refuse even for empty UPDATE requests. BeforeChange and BeforeDelete
are the only admitted event declarations; AfterDelete remains outside this profile.

BeforeChange executes for the explicitly requested insert/update operation.
Current changes sequentially; update Old stays frozen. BeforeDelete validates
under its own Old-only bindings and cannot contain SetField. Its guards do not
execute during an UPDATE or logical INSERT. Active INSERT still refuses Old.
These dispatch boundaries follow Microsoft's desktop [Before Change](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/before-change-macro-event)
and [Before Delete](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/before-delete-macro-event)
contracts. Selector/envelope order remains explicit engineering order, without a
native event-order or timing claim.

All compiled operations count toward the 4,096-operation ceiling, including
inactive operations discarded after validation. The retained matching stream
keeps original program, definition, node and source-span identities. Fresh
original-source reconstruction checks all program revisions, active execution and
joint final rows before the existing single physical update finishes. No candidate
chaining or report-to-write conversion is introduced. RaiseError returns a typed
rejection without a candidate.

Existing public guard-only preparation retains its previous selected-event
admission. Public `BeforeChangeFields` structural inspection remains Change-only;
the complete field compiler alone uses a crate-private Change/Delete inspection
entry with the same lexer, tree and action builder. Existing body/module/children,
constraints, index ownership, allocation and byte-preservation gates remain.
Native macro-location/required-event coverage, logging, handlers, Updated timing,
after-events and normal ACE saves remain unfinished.

## Observed evidence

The independent SOURCE gate passed first: 24 pairs/48 views across headers 5/6,
including separate properties, multiple envelopes in one property, reversed
disjoint selectors, sequential Change definitions, inactive Delete rejection and
invalid inactive-program/selection controls. Complete rows, constraints, index
membership, Extra bytes and original physical roles were qualified before the
new compiler ran. Source constructors and complete expected images stayed fixed.

The first normal compilation passed, but the first API round returned the public
Change-only inspector's `Unsupported` result: one function passed and three
failed. The correction added the private complete inspection entry; it changed
no fixture or expected byte and preserved the public restriction.

Corrected normal compilation, five API functions, two causal controls and the
full 21-function guard regression pass. Ten distinct changed candidates and ten
empty-operation images match complete frozen files; two budget-repeat comparisons
also match. Controls cover 24 nonempty/empty refusals, active rejection, legacy
guard refusal, exact pools/one-below, final cancellation and correlated original
request/program-revision substitutions. Logical INSERT separately checks both
headers with one supplied row containing a duplicate source ID: this is preparation only, not insertion
or constraint permission. Existing logical cases retain active INSERT Old refusal.
The public Extra regression passes all 46 functions, including the original
logical 18-case corpus. Native strict checks pass for all targets/features; the
no-default-feature check and formatter also pass. Exact logs and retained phase
failures are in the [focused record](results/ace16-before-event-collections-2026-10-30.json).
