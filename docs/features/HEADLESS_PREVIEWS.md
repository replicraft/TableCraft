# Headless application previews

Required future public services; no form/report renderer or preview command is
implemented. `tablecraft-render` owns reusable layout/measurement/rendering;
forms/reports own their domain definitions and behavior. `tablecraft-builders`
composes those services in the designers. The name remains builders, not the
older `tablecraft-design` spelling in the supplied proposal.

| Service direction | Required outputs |
| --- | --- |
| Form preview | Image plus semantic control tree with stable IDs, bindings, geometry, visibility and layout constraints |
| Report preview | Paginated document/PDF plus section/page measurements and totals context |
| Layout measurement | Overflow, clipping, overlap diagnostics, missing fonts and unsupported controls |
| Accessibility inspection | Labels, roles, focus/tab order and missing accessible names |
| Layout comparison | Structural and visual differences tied to exact source/staged revisions |

Headless and desktop designers consume the same definitions and measurements;
screenshots alone cannot describe bindings or prove accessibility. Return both
visual artifacts and machine-readable diagnostics so clients can make precise
edits. Keep printable report geometry independent of responsive UI layout.

Use explicitly supplied or authorized bounded sample data. Default preview must
not execute Python, event handlers, external resources or active Access content.
Behavioral previews require separate execution permission and disposable scenarios.
Apply read/redaction policy to images, semantic trees and PDFs alike; artifacts
can reveal private values. Project-scoped handles and download operations require
authorization, revision checks, output limits and retention/cleanup policy.

Record definition/staged revision, sample-data revision, renderer version,
platform/scale, fonts and deterministic inputs. Missing fonts or unsupported native
controls remain visible diagnostics, not silent fallback evidence of equivalence.
Set work/memory/page/image/output ceilings and cancellation checks. Visual tests
must control fonts/platform or document approved tolerances; no headless image
alone qualifies a native Access form writer or cross-platform UI behavior.

Qualify these services with [application scenarios](../testing/APPLICATION_SCENARIOS.md)
before exposing equivalent CLI/MCP/GUI adapters.
