# Explicit AllowZeroLength insertion

Normal indexed Text INSERT now has independently qualified sources with explicit
Boolean AllowZeroLength properties. This adds acceptance coverage to the existing
field-policy implementation; it introduces no new production permission or
fallback.

Both General Legacy and General Modern sources contain nullable Text, a manual
Int32 primary and ascending, duplicate-alias and descending nonunique Text
indexes. Explicit true permits an empty string. Explicit false rejects it with
the typed field constraint error, while supplied NULL remains NULL under either
policy. Empty strings are indexed values; NULL has its own carrier and existing
index policy. Original histories are preserved or advanced according to the
specific insertion, rather than recomputed from current distinct values.

Fourteen complete source views and twenty positive request contexts passed
independent modeling, inert SDK reads and full native Source checks before
matching writes. Native literal commits, independent proof/forgery checks,
recovery/resource controls and retained typed/SQL sessions pass. CLI acceptance
checks exact saves, NULL versus empty readback and no-effect policy refusals.
See the [source record](../../../tests/reports/creation/ACE16_AZL_AUTONUMBER_SOURCES.md) and
[execution record](../../../tests/results/general/ace16-explicit-text-azl-writes-2026-11-01.json).

This evidence does not grant empty-string permission when the property is absent.
The earlier four absent-property refusals remain intact. Arbitrary field-policy
expressions, indexed Text splits and native Access save/reopen remain work.
