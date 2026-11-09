# Compatibility engineering

Independent Access models, reference probes, comparison tools and retained
results live here. Rust unit and integration tests remain with their Cargo
packages; this directory adds no Cargo test targets.

| Area | Contents |
| --- | --- |
| [Access tools](access/README.md) | Feature-local Python, Java and Rust engineering tools |
| [Reports](reports/README.md) | Evidence summaries and compatibility boundaries |
| [Results](results/README.md) | Captured verification records, grouped by domain |

Run a current Python tool through the import setup:

```sh
python3 tests/run.py tests/access/queries/expressions/compare_general_like.py --help
```

Historical records retain their original paths, source hashes and observations.
Moving tools does not rerun their captured comparisons. Current tools use current
paths; there is no historical-path registry or replacement for an original hash.
Windows/Access comparisons still require the actual external application.
