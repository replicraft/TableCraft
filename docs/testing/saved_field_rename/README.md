# Same-width saved-field rename: raw mutation and inert reader evidence

This is an original first-party byte model and read-only SDK observation recipe.
Python changes generated copies; Java only opens them read-only, disables
expression evaluation and inspects metadata. There are **no SDK rename writes**
and no Microsoft Access application execution in these observations.

Four pinned nine-role, explicit-OFF sources come from the
[OFF-after-field-ADD evidence](../nac_after_field_add/README.md). They cover first
Number, second Number, Number beside Short Text, and Text→Number→Text history.
The selected ASCII names change Score→Total or Label→Title without changing their
UTF16 width. Each source has two separately frozen raw hypotheses:

* `td-only` changes the TDEF name alone. All four reopen, but SDK field-property
  lookup under the new name no longer exposes the selected original properties.
  These are negative controls, not SDK read failures or deleted property bytes.
* `live-group` changes that TDEF name and the matching group name in the **live
  user** MR2 body. All four preserve observed metadata except the requested name,
  including native number/ID, declaration, compression and typed policies.

Both variants keep original source bytes, file length, all catalog/map bytes,
old historical property rows, the separate database OFF body and user data/index
pages unchanged outside their literal name spans. The coordinated variant changes
pages 7 and 10; the control changes only page 7. The user body's actual descriptor
can be behind a forwarded catalog root and is not the final MSysDb row.

## Portable reproduction

Run from the repository root using Python 3, JDK 21 and the previously reviewed
Apache-2.0 Jackcess 5.0.4 JAR, with SHA256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
Supply a local directory containing the four generated source basenames in
`source_manifest.json`; their exact hashes are verified. No database, JAR or
compiled class accompanies this recipe. Use a fresh output directory; existing
results are refused and must be retained rather than overwritten.

```sh
python3 docs/testing/saved_field_rename/prepare.py \
  --sources-dir "$RENAME_SOURCES" --output-dir "$RENAME_OUTPUT"
python3 docs/testing/saved_field_rename/run.py \
  --sources-dir "$RENAME_SOURCES" --output-dir "$RENAME_OUTPUT" \
  --jar "$JACKCESS_JAR" --java-bin "$JAVA_HOME/bin"
```

The first command writes all eight complete raw hypotheses and their hashes before
any SDK getter runs. The second verifies frozen model/helper/JAR hashes, compiles
the original observer, records four baseline and eight variant reads, and compares
all field/table/option metadata with the baseline except the requested name.
Every open is read-only with expression evaluation disabled. All originals and
all observed copies must remain byte-identical through reading. No Cargo,
production encoder, native app, network dependency install or database content
execution is involved.

The maintained model and Java helper are byte-identical to the executed originals.
Portable prepare/run wrappers replace the original local path configuration;
they have not been executed as part of promotion. Successful SDK observations
were not repeated. The [9 November record](../results/saved-field-rename-raw-sdk-read-2026-11-09.json)
retains original/portable hashes, frozen prediction identity, all baselines, eight
positive/control results and stdout/stderr hashes. Original local results remain
preserved separately.

## Limits and provenance

The Python model uses original literal TDEF/MR2/catalog framing for these exact
pinned generated inputs. It is an experiment oracle, not a general parser or
production rename algorithm. It imports no SDK/native encoding implementation.
The original Java observer uses only the reviewed SDK's public read APIs. This
first-party source follows the repository MIT license; Jackcess retains its
Apache-2.0 provenance in [FIXTURES](../FIXTURES.md).

Readable raw mutations establish a bounded storage/lookup relationship, not what
Access or the SDK would write when asked to rename. There is no qualification of
SDK rename APIs, Microsoft Access interaction/save events, timestamps, dependency
updates, NameMaps, repeated rename, different-width/Unicode names, populated
sources, other indexes/layouts or older Jet/ACE profiles. Native candidate,
original-intent/recovery and normal GUI qualification require their own evidence;
this record does not activate a writer.
