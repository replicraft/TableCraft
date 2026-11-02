# Read-only action Source for General Text indexes

The internal action Source now reads and preserves qualified General Legacy/2010
Text indexes alongside the existing numeric/Binary families. This qualifies an
immutable source and required-program absence; it grants no mutation permission.

Only the SelectedActions declaration and retained-read classifier change. The
profile requires a single native Short Text key, variable/manual flags 2,
extended flags 0, positive even width up to 510 bytes and exact LCID 1033,
variant 0, version 0 or 1. Unknown locales/variants/versions, unsupported raw
metadata and incomplete declarations remain refused. Canonical and SelectedUser
admission keep their previous limits.

Actual NoRequired capture, complete catalog/declaration/property checks,
constraints, strict reachable/owned index closure, alias/census/map exclusivity,
LVAL inventory and raw record protection remain mandatory. Certificates stay
bound to their issuing image. Source success cannot substitute for action facts,
mutation ownership, target binding or an independent candidate proof.

The [original source phase](../../testing/ACE16_TEXT_INDEX_UPDATE_SOURCES.md)
first recorded the old `Unsupported(ActiveContent)` profile refusal separately
from body/module/Extra absence. Its code and first receipt remain unchanged in
history. Current tests explicitly replace that historical boundary with actual
Source-positive checks over 36 table contexts, plus 84 malformed/unknown/alias/
map/DATA controls, a foreign-image certificate denial and current public mutation
refusals. The initial read-only patch kept Text UPDATE/INSERT/DELETE planners
closed. The separately qualified [fitting UPDATE writer](NORMAL_TEXT_INDEX_UPDATE.md)
now consumes this Source with its own action facts, binding and proof;
INSERT/DELETE remain separate follow-ons.

These are local source/preservation checks. Native Access save/reopen and Windows
qualification remain unrun.

The [execution record](../../testing/results/ace16-text-action-source-2026-11-01.json)
retains the original refusal and new positive admission as separate observations.
