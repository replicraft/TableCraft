# Historical mixed-row Number materialization

This first-party observer exports twelve complete raw images from
`tablecraft-testkit::fixtures::access::canonical_created_number::mixed::after_off::populated_mixed`.
It never invokes TableCraft's native writer. Three source families retain Text,
Binary, or an additional absent Text column. Each has one before image and three
independent after images, materializing Bonus=37 in exactly one historical row.

From the repository root, with JDK 21 and the reviewed Jackcess 5.0.4 JAR:

```sh
python3 tests/run.py tests/access/schema/historical_mixed_number/observe.py \
  --output-dir .cache/historical-mixed-sdk-fresh \
  --jar /path/to/jackcess-5.0.4.jar \
  --java-bin /path/to/jdk-21/bin
```

The runner reuses the existing SDK runner's process evidence helper and reviewed
JAR digest (`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`).
Obtain the JAR separately from the [official Jackcess release](https://github.com/jahlborn/jackcess/releases).
No JAR, generated database, compiled observer or generated report belongs in Git.
The output directory must be new and below `.cache/`. Existing observations are
never overwritten. Cargo uses the caller's normal target-directory configuration.

The frozen input receipt precedes SDK execution. Read-only getters check declared
types and Number policies, all original Text/Binary/NULL values, the selected new
Number and remaining absent NULL fields. Sequential and primary-key index cursors
must each yield all three logical records, including moved records reached through
their original root references. All twelve whole-image hashes must remain unchanged.

Successful execution qualifies these independent SDK reads only. It does not
qualify SDK mutation, Microsoft Access execution/save, other source families or
other file profiles. Compilation failures and failed observations remain in the
chosen evidence directory; rerun into a new directory after fixing the cause.
