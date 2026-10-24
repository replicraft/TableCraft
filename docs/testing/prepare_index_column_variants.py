"""Prepare isolated ACE collection experiments without patching the working tree.

Copies only ACE sources into fresh ignored workspaces. Other local dependencies
remain read-only references to the checkout. Build/timing coordination belongs
to the caller; this command never compiles, times or edits original fixtures.
"""

import argparse
from pathlib import Path
import re
import shutil


CRATE = Path("crates/engines/tablecraft-ace")


def replace_once(text, before, after):
    assert text.count(before) == 1, before
    return text.replace(before, after)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    assert not args.directory.exists(), "Fresh experiment directory required"
    repository = Path(__file__).resolve().parents[2]
    workspace = (repository / "Cargo.toml").read_text()
    workspace = re.sub(
        r"members = \[.*?\]", f'members = ["{CRATE.as_posix()}"]',
        workspace, count=1, flags=re.DOTALL,
    )
    workspace = re.sub(
        r'path = "([^"]+)"',
        lambda match: 'path = "' + (repository / match[1]).as_posix() + '"',
        workspace,
    )
    assert "arrayvec =" in workspace and "smallvec =" in workspace
    for variant in ["vec", "arrayvec", "smallvec"]:
        destination = args.directory / variant
        destination.mkdir(parents=True)
        (destination / "Cargo.toml").write_text(workspace)
        shutil.copy2(repository / "Cargo.lock", destination / "Cargo.lock")
        shutil.copy2(repository / "rust-toolchain.toml", destination / "rust-toolchain.toml")
        shutil.copytree(repository / CRATE, destination / CRATE)
        if variant == "vec":
            continue

        manifest = destination / CRATE / "Cargo.toml"
        manifest.write_text(replace_once(
            manifest.read_text(), "[dependencies]\n", f"[dependencies]\n{variant}.workspace = true\n",
        ))
        metadata = destination / CRATE / "src/table/indexes/metadata.rs"
        definition = destination / CRATE / "src/table/indexes/definition.rs"
        source = definition.read_text()
        metadata_source = metadata.read_text()
        original = """        reader
            .budget
            .charge_allocation(INDEX_KEY_COLUMNS as u64, size_of::<IndexColumn>() as u64)?;
        let mut columns = Vec::new();
        columns
            .try_reserve_exact(INDEX_KEY_COLUMNS)
            .map_err(|_| InspectError::Allocation)?;
"""
        if variant == "arrayvec":
            collection = "ArrayVec<IndexColumn, INDEX_KEY_COLUMNS>"
            metadata_source = metadata_source.replace(
                "use super::super::schema::TableSchema;",
                "use super::super::schema::TableSchema;\nuse crate::format::layout::INDEX_KEY_COLUMNS;\nuse arrayvec::ArrayVec;",
            )
            source = source.replace("use tablecraft_schema::SchemaName;", "use arrayvec::ArrayVec;\nuse tablecraft_schema::SchemaName;")
            setup = "        let mut columns = ArrayVec::<IndexColumn, INDEX_KEY_COLUMNS>::new();\n"
            source = replace_once(source, original, setup)
            source = replace_once(
                source,
                """            columns.push(IndexColumn {
                number,
                descending: physical[at + size_of::<u16>()] & ASCENDING == 0,
            });""",
                """            columns
                .try_push(IndexColumn {
                    number,
                    descending: physical[at + size_of::<u16>()] & ASCENDING == 0,
                })
                .map_err(|_| InspectError::Unsupported)?;""",
            )
        else:
            collection = "SmallVec<[IndexColumn; INLINE_INDEX_COLUMNS]>"
            metadata_source = metadata_source.replace(
                "use tablecraft_schema::SchemaName;",
                "use smallvec::SmallVec;\nuse tablecraft_schema::SchemaName;\n\npub(super) const INLINE_INDEX_COLUMNS: usize = 2;",
            )
            source = source.replace(
                "use tablecraft_schema::SchemaName;",
                "use super::metadata::INLINE_INDEX_COLUMNS;\nuse smallvec::SmallVec;\nuse tablecraft_schema::SchemaName;",
            )
            setup = """        let capacity = (0..INDEX_KEY_COLUMNS)
            .position(|key| {
                let at = KEY_START + key * KEY_BYTES;
                physical[at..at + size_of::<u16>()] == UNUSED_COLUMN.to_le_bytes()
            })
            .unwrap_or(INDEX_KEY_COLUMNS);
        if capacity > INLINE_INDEX_COLUMNS {
            reader
                .budget
                .charge_allocation(capacity as u64, size_of::<IndexColumn>() as u64)?;
        }
        let mut columns = SmallVec::<[IndexColumn; INLINE_INDEX_COLUMNS]>::new();
        columns
            .try_reserve_exact(capacity)
            .map_err(|_| InspectError::Allocation)?;
"""
            source = replace_once(source, original, setup)
        definition.write_text(source)
        metadata.write_text(replace_once(metadata_source, "Vec<IndexColumn>", collection))
    print("Prepared three isolated native metadata variants; original sources unchanged.")


if __name__ == "__main__":
    main()
