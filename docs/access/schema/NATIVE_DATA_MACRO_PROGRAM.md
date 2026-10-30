# Inert data-macro program structure

`TableExtraProperty::inspect_macro_program` inspects an explicitly selected
structural profile without evaluating expressions or executing actions. The
initial `MacroProgramProfile::DocumentedDesktop` recognizes Statements,
ConditionalBlock/If/Condition and the documented desktop RaiseError argument
shape. This is an inspection capability, not macro execution, complete semantic
detection, an absence certificate or a normal-write permit.

The separate [explicit before-event guard preparation](NATIVE_BEFORE_EVENT_GUARDS.md)
consumes selected source-owned programs and typed intents. It does not run during
inspection or turn structural coverage into native event/save authority.

## Explicit consumer

```sh
tablecraft inspect example.accdb --table Orders --extra --macro-program
```

The flag requires `--extra` and `--table`, and conflicts with `--macro-xml`.
Framing-only Extra and the existing XML-envelope mode retain their behavior.
The new report includes envelope identities, structural coverage, node/parent
indices, opaque diagnostics and source spans. It prints no condition, argument or
raw XML body text. True lexical/resource failures leave buffered stdout
unpublished, including when a later property fails after an earlier valid one.

Names-only bags and empty decoded groups report no program documents inspected,
explicitly without certifying absence. Missing, NULL, empty or opaque Extra still
refuses the selected decoder. No stored content opens a resource or invokes a
provider.

## Selected profile and opaque structure

The caller selects the documented desktop profile; it is not inferred from the
file selector, property name, numeric group, Event string or namespace alone.
The [existing namespace families](NATIVE_DATA_MACRO_XML.md) remain explicit.
Descendant prefixes are resolved by namespace URI. Equivalent aliases work;
foreign/rebound elements and qualified attributes stay opaque even when their
local names resemble known structures.

Microsoft's desktop RaiseError contract requires an Error Number and permits an
optional Description. Its BeforeChange XML spells those arguments Number and
Description. This profile checks those element shapes and cardinalities only.
A present-empty element differs from an absent element. No cardinality result
establishes a Long, Boolean, nonempty expression, field binding or execution
result. [Desktop RaiseError](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/raiseerror-macro-action),
[BeforeChange XML](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/before-change-macro-event)

Missing, repeated or unfamiliar arguments retain an opaque action rather than
selecting a first/last value or declaring every such document malformed XML.
Unfamiliar actions, attributes, nesting and unsupported conditional branches also
retain whole opaque subtrees with diagnostics. Empty Statements remains an
explicit macro envelope; it does not mean the table has no macros. Event/name
facts are retained without selecting executable event semantics.

Services AXL2 RaiseError lists only Description, so that reference does not define
the desktop argument profile. SetField stays opaque: the desktop BeforeChange
example uses it directly, while generic action and Services documentation impose
CreateRecord/EditRecord context. This implementation does not invent one
universal execution rule. [Services RaiseError](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-axl2/212dd877-a6f7-45e1-9f33-2d7a33ce5e5d),
[custom web app RaiseError](https://learn.microsoft.com/en-us/office/client-developer/access/raiseerror-macro-action-access-custom-web-app),
[SetField context](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/setfield-macro-action)

## Exact source and bounds

The result borrows its original property owner. Native property bytes remain
available separately, and the containing TableExtra retains the complete outer
LvExtra payload. Nothing is reconstructed from the recognized nodes.

`XmlByteSpan` uses the exact original **decoded XML UTF-8 input buffer**, including
any leading BOM. It is not a UTF-16 property, compressed/assembled LvExtra, LVAL
page or database-file offset. Whole elements and raw inner-XML contents retain
separate ranges. Entity/CDATA contents are not evaluated or silently substituted
into the coordinate system. A present-empty range has equal start/end offsets;
a missing optional element is None.

The existing lexical reader remains the XML name, declaration, namespace,
character and entity safety owner. Program inspection first completes that
validation, then builds a separately paid bounded source-position arena from the
same immutable text. Unknown subtrees still undergo lexical checks. DTDs, custom
entities, processing instructions, unbound prefixes and malformed XML refuse
without external resolution. The envelope-only path retains its own strict
namespace behavior.

Limits are 2 MiB XML input, depth 128, 64 attributes per element, 128 namespace
bindings and 1,024 definitions. Program inspection additionally bounds staging
at 16,384 XML elements, retained output at 4,096 nodes, and known-namespace Action
containers at 64 direct child elements, including unknown actions. Allocations
are fallible and charged before reservation. Parsing/traversal and final
publication check work and cancellation. The property decoder consumes no row
budget; native source capture retains its separate row/read limits.

## Explicit text extraction

`DataMacroProgram::text_content` materializes a selected recognized Condition,
Number or Description through `MacroProgramTextKind`. Callers select a node and
slot, not an arbitrary XML fragment. The returned `MacroProgramText` retains the
original `source_span()` separately from its decoded `text()`. Wrong node/slot
combinations and opaque nodes refuse. Missing Description returns None; a
present-empty argument returns an empty text value.

Extraction decodes predefined and numeric references once, joins text and CDATA,
and excludes XML comments. XML 1.0 literal CRLF/CR become LF; a referenced
`&#13;` remains a carriage return. Unicode, ordinary whitespace, expression
spelling and element-leading U+FEFF are preserved. A document BOM remains part of
the original coordinate buffer. Raw property/XML bytes and spans are unchanged.
The existing XML safety owner supplies reference validation and the bounded
text reader; no external entities or resources are resolved.

This does not parse expressions, coerce argument types or execute macros. Plain
Description text and leading-equals expression spelling remain distinct input
for future explicit execution. Text extraction uses cumulative byte/work pools,
fallible output reservation and cancellation through final publication; it
consumes no row allowance. The existing XML input ceiling also bounds output.

The dedicated source gate passed two functions before extraction API execution:
32 BOM/no-BOM carriers with 96 exact raw text slots, three absent/present-empty
carriers, and two opaque controls. Four focused API functions then passed on their
first run, together with the two source functions. They compare independently
literal decoded strings, original spans/bytes, absent/empty distinctions,
wrong/opaque selections, resource refusals, terminal cancellation, repeated
leading U+FEFF and larger output. Subsequent mechanical changes call the actual
text owner directly without a forwarding re-export and add a payload-free tracing
span at the public boundary; no decoding behavior or golden changed.
The broader native Extra target passes 29 functions, and the existing CLI Extra
target passes 13, including all 50 structure carriers. Strict ACE/CLI
all-target/all-feature Clippy and the minimal ACE build pass on their first
invocations. Checks for this additive extraction slice are recorded in its
[focused result](../../testing/results/data-macro-program-text-2026-10-30.json).

## Observed verification

### Source witnesses for before-event preparation

Separate source-only tests now qualify 40 literal NativeImage carriers across
both ACE16 layouts. The same captured image supplies the scoped local table,
three original Int32 rows and its exact catalog-owned MR2/UTF-16 program bytes.
Existing typed update/insert/delete requests and 20 independent trace/outcome
cases are retained as future execution expectations. These source checks do not
evaluate those programs or establish passing execution semantics.

An additional selector witness passes six admitted MR2 sources and two native
Malformed controls. It records exact group-kind/property-name scope, caller
engineering order, two independently authored properties and unselected
opaque/malformed XML. Its first source attempt incorrectly expected repeated
parsed group-kind 3 to be admitted; the existing MR2 decoder already rejects
that source. The corrected test retains the same negative data, expects native
Malformed and adds phase diagnostics. Original inputs/logs and all 20 original
program/trace bodies are preserved; no decoder was weakened.

These witnesses do not qualify runtime ambiguity selection, native macro storage
discovery, required-macro coverage or native dispatch order. The guard evaluator
and host consumer remain uninstalled and unexecuted at this source prerequisite.
See the [source-only result](../../testing/results/ace16-guard-sources-2026-10-30.json).

### Structural inspection

The source gate passed 25 independently authored native MR2/property/XML carriers
and 42 literal node spans before program API execution. They cover both observed
namespaces, prefixes/rebinding, BOM and multibyte/CDATA input, nested If,
present-empty/missing/repeated arguments, untyped contents, opaque nodes and
lexical errors. No third-party example program was copied into these fixtures.

The first native API target passed all 13 functions: five program controls and
eight existing envelope regressions. The broader table-Extra target passed 23
functions; strict ACE all-target/all-feature Clippy and a minimal ACE build pass.
Budget/ceiling refusals, original-byte retention, and early/measured terminal
cancellation return no partial AST. All frozen structures and spans are unchanged.

The CLI source gate then passed 50 complete carriers across both ACE16 layouts,
before consumer API calls. Its first API invocation passed three functions and
failed one test helper: the helper assumed blank TSV section separators and
mistakenly included following summary rows. The node rows already matched.
Only the test reader/imports changed, using the existing quoted-TSV CSV parser
and adding a quoted-tab/newline control; decoder, consumer and goldens stayed
unchanged. The corrected target passes all five functions, including the full
50-carrier matrix. Those calls produce 38 structural reports and 12 atomic
refusals. Broader CLI checks pass 37 unit and 37 selected integration functions;
strict CLI all-target/all-feature Clippy passes on its first invocation.

Current tests do not qualify macro execution, old/new/IsInsert/Updated binding,
transaction effects/rollback, complete native storage detection, native ACE16
absence, or Microsoft Access save/reopen. These remain required work under
[data-macro acceptance](../../testing/DATA_MACROS.md). The separate
[historical envelope proof](../../testing/ace16_data_macro_xml/README.md) retains
its original consumer/source witnesses; it is not current program qualification.
See the [focused result](../../testing/results/data-macro-program-inspection-2026-10-30.json).
