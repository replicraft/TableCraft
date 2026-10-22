# CurrentUser and explicit Access session data

Parenthesized, unqualified `CurrentUser()` now runs in scalar and bound queries.
It materializes the exact account name supplied by an immutable
`AccessSessionContext`. Libraries require that context explicitly through
Evaluator::with_access_session or ExecutionOptions::access_session; omission
returns AccessSessionRequired. Plans retain no account data between executions.

Microsoft's [CurrentUser method reference](https://learn.microsoft.com/en-us/office/vba/api/access.application.currentuser)
defines the Access account name and its default Admin account when there is no
security-enabled workgroup. The value is distinct from an operating-system,
network or Office account name. Microsoft's
[user-level security reference](https://support.microsoft.com/en-us/access/set-or-change-access-2003-user-level-security-in-current-access-versions)
separates legacy MDB workgroups from ACCDB formats, which do not support that
security model. These are reference contracts, not native comparison evidence.

## Session and permission boundary

The CLI explicitly composes an unsecured Access expression session for each
`query eval` and `query run` invocation. Its CurrentUser value is `Admin` for
inline, native MDB/ACCDB and SQLite-backed queries. This describes the chosen CLI
expression session. It does not infer a login from a database header, filename,
encryption password, environment variable or the machine's active account.

```sh
tablecraft query eval "SELECT CurrentUser(),Eval('CurrentUser()')"
tablecraft query run "SELECT ID,CurrentUser() FROM ScalarSamples ORDER BY ID" --database example.accdb
```

AccessSessionContext::unsecured declares that default session. A library host may
instead issue AccessSessionContext::new(name), retaining its own account-name
assertion. This is bounded expression metadata: it is not an authenticated user,
permission principal, database capability or proof of MDW login. There is no
CLI account-override flag, workgroup reader/authenticator, ACL enforcement or
mapping to TableCraft's future application-security identities. Declaring Admin
grants no new read/write privileges and does not qualify opening a secured
workgroup database in Access. Existing native editing constraints remain intact.

The borrowed name is immutable for the execution. Construction rejects empty or
control-containing names and caps them at 65,536 UTF-8 bytes; that is a bounded
host-data profile, not a claim about native MDW account-name syntax or length.
No trimming, normalization, case folding, ANSI conversion or localization occurs.
The context has no Debug/Serde implementation that could expose the name
implicitly. Its function slice lives in the expression library; no empty security
or session crate was introduced.

## Evaluation and query behavior

The evaluator checks cancellation/work before reading its supplied data, then
uses ordinary charged, fallible Text allocation and its caller-lowerable ceiling.
Repeated calls consume the same allowance. Wrong arities, wildcards, missing slots
and qualifiers fail before supplied operands execute. No provider callback or
OS lookup is performed. Bare property syntax is not implemented by the function
lookup, and restricted UPDATE binding does not gain session calls.

Nested, named, correlated, domain and closed Eval expressions inherit the same
context. CurrentUser is repeatable within an execution, so structural grouping
and pivot keys can use it; closed Eval key validation allows the immutable read.
Returned Text has fresh host-default provenance. Text comparison/grouping still
requires an appropriate explicit host/database collation; the account string is
not silently compared with OS locale rules. Materializing a session value does
not store the context or activate database/VBA content during inspection.

## Evidence and remaining work

One parser-free context control covers exact Unicode/whitespace/case retention,
invalid names and independent byte boundaries. Two scalar controls cover missing
context, exact UTF-8 output charges, narrower Text ceilings, shared dynamic context
and every observed cancellation checkpoint. Three bound controls cover plan reuse
with different names, grouping, domains, derived/correlated queries, wrong arities,
late operational failures, quotas and all observed nested cancellation checkpoints.
CLI checks set USER/LOGNAME/USERNAME to a different account and still receive the
explicit Admin session, with empty stdout on late failures.

The original [session runner](testing/compare_current_user.py) passes 58 controls for the declared
unsecured profile, type/arity behavior and native-field/group/domain integration.
Reviewed getters supply IDs from four unchanged ScalarFixture files with
expression evaluation disabled. It does not run CurrentUser in Access or prove
workgroup authentication/permissions. Native account-name, session/host invocation,
saved/application and workgroup qualification remain pending. CurrentUser is
Partial; full legacy user-level security remains separate backend work.

The optional SQLite worker accepts owned AsyncOptions::access_user_name metadata
and validates it through the same constructor before query execution. Its default
remains no session; it does not infer Admin or retain a previous request's name.
AsyncOptions::random_seed similarly creates a request-local checked Rnd state,
with no implicit provider when absent. An all-feature worker check exposed stale
execution-option initialization; the repaired worker also explicitly leaves field
collation resolution unselected. A reuse control covers names, seeds, missing
providers, invalid controls and successful cleanup. These options add no SQLite
authentication or write capability.
