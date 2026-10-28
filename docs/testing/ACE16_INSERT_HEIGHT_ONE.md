# Fitted INSERT into an existing two-leaf primary

The optional [ordinary-row INSERT preview](ACE16_INSERT_PREVIEW.md) also admits
an original height-one Int32 primary with exactly one bounded left leaf and one
right tail leaf. This extends the same `preview_ace16_insert_rows` operation;
it introduces no separate public API, normal write path or journalable plan.
The existing explicit Int32/Short Binary record profile and all required,
unique, metadata, property and resource gates still apply.

## Original routing and capacity

The original root must contain one complete Int32 key/ROW separator pointing to
the left leaf, and a tail pointing to the right leaf. Both children are existing
leaves with the exact reciprocal sibling chain. The selected physical ownership
map contains exactly those three index pages. True logical aliases share the
same physical identity.

The continuation retains every original key/ROW pair and routes the sorted
request-derived final stream against the original inclusive separator. Pairs
at or below that complete bound stay on the left; the rest belong to the right
tail. This is INSERT routing, rather than UPDATE's partition by original leaf
counts. The original separator remains the left maximum. The root's entire page,
including header, separator, child identities and unused residue, remains
byte-identical.

Both final leaves must fit under their original prefix seed, reduced to the
new full key/ROW common prefix and repaired further only when capacity requires
it. Both capacity decisions complete before any leaf patch is returned. Only
changed leaves are rewritten. Their header links remain unchanged, their bitmap,
free count and encoded entries have exact expected bytes, and the original
unused suffix stays in place. An untouched leaf remains identical in full.

This branch allocates no index leaf, node or bitmap, changes no index ownership
map and moves no index page. A leaf that would overflow refuses before returning
a candidate; this branch does not split it or grow the parent. DATA placement
can independently reuse space or append under the existing qualified ordinary
allocation rules. Table and historical physical-primary counters increase by
the inserted row count once per physical index, including logical aliases.

## Independent final proof

The audit decodes the original root and both leaves directly, checks the full
separator, original owned-page set and sibling geometry, and reconstructs the
original complete key/ROW stream. The parent binds new pairs to immutable row
requests and independently certified visible DATA ROWs. The audit repartitions
that complete final stream against the original bound and uses its own retained
wire renderer to derive both leaf images. It does not consume builder maxima,
leaf partitions, capacity decisions or serialized output as authority.

The final proof requires the exact changed-leaf patch inventory and no index
allocation, rebase or promotion inventory. It protects the entire root and each
untouched leaf, checks the final DATA-phase append cursor and aggregate map
ledger through the common INSERT proof, then compares the complete candidate
image. Source, request or staged-plan drift cannot grant extra byte ownership.
Empty qualified requests retain all source bytes and emit no index patches.

Literal controls cover empty, left-only, right-only and bulk insertions across
both ACE 16 header profiles, including logical aliases and unchanged maps.
Adversarial controls first prove healthy candidates and checked semantic contents,
then require refusal for root residue changes, untouched-leaf residue changes
and request drift. These are engineering literal checks. New original-file
candidate/getter qualification for this branch has not yet been performed;
the earlier [root-leaf growth evidence](ACE16_INSERT_ROOT_SPLITS.md) remains
separate. Neither those Jackcess observations nor these literals establish
native Microsoft Access save/reopen compatibility.

## Remaining boundary

The source must have exactly the admitted height-one, two-leaf form. Additional
children, deeper trees, child overflow requiring allocation, broader index
families, generated/defaulted values, required data macros and normal native
saves remain separate work. Existing root-leaf fitting and two-fresh-leaf growth
behavior is retained.
