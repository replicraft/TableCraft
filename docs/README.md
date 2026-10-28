# Documentation

Start with the [compatibility matrix](COMPATIBILITY.md),
[current implementation](IMPLEMENTATION.md) and [roadmap](ROADMAP.md).
Subsystem documents distinguish working behavior, explicit limits and planned work.
The current [product priority](decisions/0003_ACCESS_BEHAVIORAL_PARITY.md) is faithful
native Access behavior; SQLite and native-project implementation are paused.

| Area | Contents |
| --- | --- |
| [CLI](cli/README.md) | Commands, browsing, parameters and streaming output |
| [Native Access](access/README.md) | Jet/ACE storage, indexes, queries, collections and writes |
| [Query engine](query/README.md) | Parsing, binding, execution, keys and aggregates |
| [Expressions](expressions/README.md) | Built-ins, numeric/text/date behavior and formatting |
| [Foundation](foundation/README.md) | Shared values, schemas, database contracts and I/O |
| [Architecture](architecture/README.md) | Package boundaries and application contracts |
| [Package formats](formats/ACCESS_PACKAGES.md) | Inert Access template and deployment codecs |
| [Development](development/README.md) | Toolchain, dependencies, tracing and releasing |
| [Other backends](backends/README.md) | SQLite integration |
| [Application features](features/README.md) | Application and interoperability contracts |
| [Testing](testing/README.md) | Acceptance scope, fixture provenance and verification evidence |

Project container specifications remain in [formats](formats/), accepted decisions
in [decisions](decisions/), and protocol contracts in [interfaces](interfaces/).
