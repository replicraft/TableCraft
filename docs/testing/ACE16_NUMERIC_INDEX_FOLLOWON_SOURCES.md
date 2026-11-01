# ACE 16 numeric index follow-on sources

This source tranche separates native read coverage from normal mutation admission. It covers exact Int64 and Currency carriers, nullable keys, empty tables, wide primary keys, stored column IDs and sparse native field ordinals. Independent-reader results establish observed reads; they do not establish native Access save compatibility or authorize mutation.

| Source family | Evidence | Scope |
| --- | --- | --- |
| Fresh SDK originals | 2 files, 12 selected table/header contexts; 76 exact read observations | All-NULL keys, wide primary keys, and stored column IDs 0/3/4 |
| Independent derivatives and empty follow-ons | 44 complete images; 352 exact read observations | Qualified DELETE-all images reused as BEFORE, authored all-NULL keys, and native ordinal gaps |
| Fresh-original follow-ons | 48 selected views, including 12 original contexts and 36 literal AFTER images; 384 exact read observations | INSERT NULL/zero families and DELETE-all across the six fresh table schemas |
| Crossing-byte ordinal derivatives | 16 complete images; 128 exact read observations | Native numbers 0/8/9 with maximum count 10 and two-byte presence masks |

The 12 original contexts occupy two SDK-created database files, rather than twelve independently generated files. The fresh follow-on BEFORE copies retain those same original bytes. Source constructors and complete report predictions froze before execution; no normal writer, serializer result or SDK mutation output supplied expected AFTER bytes.

All four native source gates passed on their first runtime attempts: 120 selected source contexts, including the 12 original table/header contexts. All 940 bounded SDK read observations and saved verifications passed. Generic rows, constraints and complete selected index membership qualified in every context. The complete ordinary Source qualifier accepted 68 selected dense contexts and deliberately refused 52 selected stored-ID/native-ordinal gap contexts. The derivative gates also verified the unmodified companion tables. Mutation tests and matching writer admission remain separate results.

The authored all-NULL derivatives retain index history 3 from their original source. Fresh SDK all-NULL indexes have observed history 1. NULL is a stored key family; insertion adds to history only when the family was absent from the current BEFORE. DELETE preserves historical counters, unused index tails, maps, properties, unrelated tables and EOF.

Stored IDs and native field ordinals are different facts. The SDK-created `StableIds` tables have dense native numbers 0/1/2 and stored IDs 0/3/4. The independently authored sparse derivatives instead declare native numbers 0/3/4, maximum column count 5, record column count 5 and presence masks `0x19`/`0x11`; their fixed payload offsets remain unchanged. Numeric index declarations reference native field 3. These sparse images are derivatives, never described as SDK-created originals. A second derivative moves native/stored IDs to 0/8/9, maximum count 10, and independently relocates 19-byte records into 20-byte records with masks `01 03` or `01 02`. This positively qualifies the second mask byte through actual SDK and native reads; stable-ID gaps alone do not establish that behavior.

Current ordinary Source qualification refused the stored-ID and native-ordinal gap profiles with `Unsupported(ActiveContent)`. That error denotes inability to establish the current closed Source contract; it is not evidence that malicious content was detected. Generic table, constraint and index reads must qualify independently before such a refusal is counted as the intended boundary. Future mutation support requires explicit metadata interpretation and real native-number-to-position mapping.

All-NULL and empty schemas retain declared numeric type information even without present key cells. Wide-primary source images encode exact signed 8-byte primary values and optional secondary keys, with unchanged four-byte logical ROW suffixes and ascending ROW tie order. Currency values are scaled Int64 units with native Currency identity; they are not coerced into Int64 values.

All bounded SDK executions passed on their first read attempt with frozen inputs and full report comparisons. Before the fresh follow-on SDK calls, a static review caught stale summary counts in the saved verifier; the original verifier was retained and a successor corrected only 44/44/264 to 48/48/288. The 384-call validation, source bytes and expected reports were unchanged. The earlier initial numeric original validator failure remains in the preceding source evidence; its inactive NULL-byte assumption was not repeated here.

Terminal native logs are `extra-originals-native-first.log`, `followon-native-first.log`, `extra-followon-native-first.log` and `cross-byte-source-native-first.log` under the qualification directory’s `root-integration/`; exact hashes are in the accompanying result record. The pending documentation predecessor is retained privately.

Native Access execution, mutation compatibility and broader sparse/default/generation policies remain unqualified by this source record.
