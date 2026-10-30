# Inert data-macro XML envelopes

`TableExtraProperty::inspect_macro_xml` explicitly inspects one property's
validated text as a bounded data-macro XML document. It returns normalized XML
`Event` or `Name` attributes and the recognized namespace, in document order.
It does not infer these facts from the native property name or numeric group.
Original bytes remain in the owning [table Extra inspection](NATIVE_TABLE_EXTRA.md).

This is envelope metadata, not a macro program interpreter, body-schema
validator, absence certificate or write permit. Unknown actions and expressions
inside an otherwise supported envelope remain uninterpreted. Event strings are
retained rather than silently mapped to executable triggers. Required table-macro
semantics remain a prerequisite to normal ACE writes.

The separate explicit [program-structure inspection](NATIVE_DATA_MACRO_PROGRAM.md)
now retains documented desktop Statements/If/RaiseError shapes and opaque
subtrees with original XML byte spans. Envelope-only inspection stays unchanged;
structural coverage still supplies no execution or absence authority.

## Explicit CLI consumer

```sh
tablecraft inspect example.accdb --table Orders --extra --macro-xml
```

This adds namespace, event and named-macro columns to the explicit Extra report.
Action bodies and expressions are not printed or executed. Plain `--extra` keeps
its framing-only behavior. Unsupported or malformed selected XML returns an error
with buffered stdout unpublished; it is not silently skipped. The flag requires
`--extra`, which requires `--table` and excludes other inspection modes.

Valid names-only bags and empty decoded groups report zero XML documents
inspected, explicitly without certifying absence. Missing, NULL, empty or opaque
payload coverage refuses this XML operation; plain `--extra` still reports those
physical states.

## Supported envelopes

Both a single `DataMacro` root and a `DataMacros` wrapper containing direct
`DataMacro` children are recognized. Supported namespace URIs end in
`accessservices/2009/04/application` or `accessservices/2009/11/application` under
`http://schemas.microsoft.com/office/`. Prefixes are resolved by namespace URI;
mixed/unknown element namespaces and unknown envelope attributes refuse.

Each definition requires exactly one nonempty, unqualified `Event` or `Name`
attribute. The parser retains normalized attribute values and does not require
them to equal the native property name. A wrapper cannot masquerade as an event;
empty wrappers and nested macro definitions refuse. Body elements remain
uninterpreted, although namespace, name, nesting, character and attribute checks
still apply throughout the selected document.

The native carrier has already decoded strict UTF-16LE. An optional leading BOM
and XML 1.0 declaration are accepted; UTF-8, UTF-16 and UTF-16LE declarations do
not select a second byte decoder. Other versions/encodings, misplaced or extended
declarations, processing instructions, DTDs and custom entities refuse. Only the
five predefined XML entities and legal character references are allowed. No
resolver opens files, URLs or application objects.

The existing workspace `quick-xml` dependency supplies namespace-aware events.
The owning reader additionally validates XML names, legal characters, expanded
attribute uniqueness and declaration order. Escaped or whitespace-normalized
namespace URI declarations remain outside this resolver profile and refuse.
This is not a claim of general-purpose XML validation or complete Access macro
schema coverage.

## Limits

The input ceiling is 2 MiB of validated UTF-8 text, independently of native payload
and cumulative read limits. Nesting is bounded at 128, attributes per element at
64, user namespace bindings in scope at 128 and definitions at 1,024. Conservative
parser allocations, retained strings/vectors, namespace resolution/comparisons
and traversal work consume the supplied budget. Parsing and final publication
check cancellation; failures return no partial document. Output has the CLI's
separate existing limits.

## Reference and qualification boundary

[Microsoft's desktop Before Change example](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/before-change-macro-event)
uses the wrapper and April namespace. The privately retained desktop-origin
payloads described in [fixture provenance](../../testing/FIXTURES.md#private-desktop-data-macro-carrier-investigation)
use single roots and the November namespace. They use ACE 2010 stored selectors,
not ACE 16 native qualification. The
[Access Services data-macro specification](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-axl2/bfdd300c-7b9c-4a9a-8669-c3cad1f3a8ce)
describes named/event concepts and rollback, not a complete native ACCDB locator
or proof of desktop runtime behavior. XML name and attribute rules follow
[XML 1.0](https://www.w3.org/TR/xml/#NT-Name) and
[Namespaces in XML](https://www.w3.org/TR/xml-names/#uniqAttrs).

Eight focused engine tests cover supported metadata, independent property/event
names, both wrappers/namespaces, inert body content, malformed/unknown input,
resource ceilings and final cancellation. Native event semantics, qualified
absence and native Access save/reopen remain unqualified; follow
[data-macro acceptance](../../testing/DATA_MACROS.md). Eight CLI tests additionally
cover explicit selection, zero-document reporting, refusal and late-error atomic
output. Seven first original-file calls match all 130 source-derived TSV records,
including nine envelopes and two names-only results with unchanged original hashes.
The [record](../../testing/results/data-macro-xml-envelopes-2026-10-29.json) pins
the captured inputs, consumer and outputs.

The maintained [saved-evidence verifier](../../testing/verify_data_macro_xml.py)
reconstructs all expected records from pinned original observations before
reading captured stdout. It launches no process and uses a retained copy of the
exact consumer; normal rebuilds need not preserve `target/debug/tablecraft`.
Missing consumer/output evidence fails explicitly.

```sh
python3 -B docs/testing/verify_data_macro_xml.py
```
