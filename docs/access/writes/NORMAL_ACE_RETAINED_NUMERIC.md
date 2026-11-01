# Ordinary updates beside unchanged numeric indexes

The native seventeen-field catalog family now permits normal fixed updates to
unindexed fields while retaining single-column Byte, Int16, Int32, Int64 and
Currency indexes. This is read qualification of unchanged indexes. It adds no
Byte/Int16/Int64/Currency indexed-target mutation, index serialization, growth,
relationship action or program admission. The initial global closed-content
family retains its existing Int32 index policy, including empty/all-NULL trees.

## Admission and preservation

Selected declarations retain exact native/scalar types, fixed widths, field
numbers and extended flags. Local ordinary/primary indexes retain their raw
physical/logical descriptors, names, flags and historical counters. Foreign,
ignore-NULL, composite and other key profiles remain refused by this extension.
Logical aliases must agree on physical identity, root, map, flags and components.

The internal complete-owned consistency check validates keys, live ROWs,
constraints, leaf chains and exact visited/owned page membership. Every index
must be checked. A separate actual catalog census rejects selected INDEX members
shared with another physical index, DATA, LVAL, header, TD or map storage.
Canonical physical aliases count once; candidate-map membership remains a hint.
The existing live LVAL inventory excludes references beyond declared owned maps.
All scans, comparisons, vector growth and cancellation checks share the operation
budget; errors never trigger a sequential or legacy-certifier retry.

Normal Source still checks properties, required programs, dependencies,
relationships, records and physical protection. The unindexed update changes
only its qualified fixed payload/mask; complete-image proof retains every INDEX
byte, map, history value and unrelated object. The old Int32/complex certifier
remains the selected path for its existing profiles. Positive-program and
engineering-preview families receive no wider admission from this change.

## Capability, consumers and recovery

Fixed capability discovery searches eligible unindexed fields before attempting
its indexed-field service. An indexed ID appearing first cannot hide a writable
unindexed field later in the declaration. A failed selected check still propagates;
there is no error-based fallback. Indexed wide targets remain refused, including
supplied empty groups, and indexed Int32 targets beside wider indexes do not
implicitly gain a new maintenance profile.

The existing typed/SQL retained-session and `query journal-update` paths consume
the same native planning service. TCNI0004 remains original fixed request intent;
fresh recovery repeats admission and independent complete-image reconstruction.
No new intent version or command is introduced.

## Evidence

The [genuine sources](../../testing/ACE16_RETAINED_NUMERIC_SOURCES.md) independently
qualify eight original/request-only `Marker` updates on two unchanged originals.
Source reads and independent SDK checks precede save acceptance. Eight native commits, eight typed and eight SQL retained-session saves, and
eight CLI saves/readbacks match the complete expected files. Twenty-four repeated
recovery contexts, stale requests and protected AFTER forgeries pass. The update
differs in one byte; partial journal states are checked without claiming a third
mixed target image. Source admission has exact/one-less resource and cancellation
controls. All 1,099 ACE unit tests, 83 recovery regressions, 79 adapter units and
94 CLI units pass, alongside strict feature builds. See the
[write record](../../testing/results/ace16-retained-numeric-writes-2026-11-01.json). Microsoft Access save/reopen and mixed-client qualification remain
unrun; this is not general native save compatibility.
