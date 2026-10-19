"""Compare original saved SELECT reconstruction with independent SQL rendering."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import roots, run
from compare_complex_links import column_layout
from compare_decimal_relationships import variable_start
from compare_attachment_columns import fields
from compare_saved_queries import catalog_ids


def oracle(args, path, name=None):
    command = ["java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
        "docs/testing/SavedSelectOracle.java", path]
    return run(*(command + ([] if name is None else [name])))


def reconstructed(args, path, name):
    return run(args.example, path, "--query=" + name)


def rejected(args, path, name):
    result = subprocess.run([str(args.example), str(path), "--query=" + name], capture_output=True, text=True)
    if result.returncode == 0 or result.stdout:
        raise RuntimeError("Unsupported/corrupt saved SELECT published syntax")


def compare(args, path, name):
    # The independent renderer supplies SQL; the already qualified syntax adapter
    # normalizes it. Native fragment assembly is performed independently on each side.
    expected = run(args.cli, "query", "parse", oracle(args, path, name))
    actual = reconstructed(args, path, name)
    if expected != actual:
        raise RuntimeError("Saved SELECT owned syntax differs: " + name)
    return actual


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "example", "ace2016", "ace2019", "sample"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path in (args.ace2016, args.ace2019):
        original = path.read_bytes()
        names = oracle(args, path).splitlines()
        if len(names) != 13:
            raise RuntimeError("Original saved SELECT count changed")
        canonical = "".join(name + "\t" + compare(args, path, name) for name in names)
        ids = catalog_ids(args, path)
        definition = ids["MSysQueries"]
        object_offset, _, columns = column_layout(original, definition, "ObjectId")
        layout = fields(original, definition)
        def fixed_offset(name, width):
            at = layout[name][0]
            if not original[at + 15] & 1 or int.from_bytes(original[at + 23:at + 25], "little") != width:
                raise RuntimeError("Expected qualified fixed query metadata field")
            return int.from_bytes(original[at + 21:at + 23], "little")
        attribute_offset = fixed_offset("Attribute", 1)
        flag_offset = fixed_offset("Flag", 2)
        selected = [slot for slot in roots(original, definition) if int.from_bytes(
            original[slot[0] + 2 + object_offset:slot[0] + 6 + object_offset], "little", signed=True) == ids["Columns"]]
        by_attribute = {}
        for slot in selected:
            by_attribute.setdefault(original[slot[0] + 2 + attribute_offset], []).append(slot)
        if len(by_attribute[6]) != 2:
            raise RuntimeError("Original projection mutation controls missing")
        cases = []
        changed = bytearray(original)
        changed[by_attribute[6][1][0] + 2 + attribute_offset] = 200
        cases.append(("unknown fragment", changed))
        changed = bytearray(original)
        changed[by_attribute[1][0][0] + 2 + flag_offset:by_attribute[1][0][0] + 4 + flag_offset] = (3).to_bytes(2, "little")
        cases.append(("conflicting query type", changed))
        changed = bytearray(original)
        changed[by_attribute[3][0][0] + 2 + flag_offset:by_attribute[3][0][0] + 4 + flag_offset] = (64).to_bytes(2, "little")
        cases.append(("unknown SELECT flag", changed))
        changed = bytearray(original)
        changed[by_attribute[6][1][0] + 2 + attribute_offset] = 8
        # WHERE cannot carry the stored output alias; it must be rejected, not dropped.
        cases.append(("unhandled WHERE alias", changed))
        order_variable = layout["Order"][3]
        order1 = variable_start(original, by_attribute[6][0], order_variable, columns)
        order2 = variable_start(original, by_attribute[6][1], order_variable, columns)
        if original[order1:order1 + 4] != b"\0\0\0\1" or original[order2:order2 + 4] != b"\0\0\0\2":
            raise RuntimeError("Unexpected original four-byte ordering profile")
        changed = bytearray(original)
        changed[order2:order2 + 4] = original[order1:order1 + 4]
        cases.append(("duplicate projection order", changed))
        changed = bytearray(original)
        changed[order1:order1 + 4], changed[order2:order2 + 4] = original[order2:order2 + 4], original[order1:order1 + 4]
        cases.append(("reversed projection order", changed))
        changed = bytearray(original)
        number = layout["Order"][2]
        mask = by_attribute[6][1][1] - (columns + 7) // 8
        changed[mask + number // 8] &= ~(1 << (number % 8))
        cases.append(("NULL projection order", changed))
        changed = bytearray(original)
        changed[definition * 4096 + 16:definition * 4096 + 20] = (len(roots(original, definition)) + 1).to_bytes(4, "little")
        cases.append(("late declared count", changed))
        with tempfile.TemporaryDirectory(prefix="tablecraft-saved-select-") as directory:
            copy = Path(directory) / "copy.accdb"
            for _, changed in cases:
                copy.write_bytes(changed)
                rejected(args, copy, "Columns")
        for name in ("Items", "missing"):
            rejected(args, path, name)
        if path.read_bytes() != original:
            raise RuntimeError("Original SELECT fixture changed")
        print(path.name, "saved SELECTs", len(names), "corrupt copies", len(cases),
            "file", hashlib.sha256(original).hexdigest(), "syntax", hashlib.sha256(canonical.encode()).hexdigest())
    original = args.sample.read_bytes()
    name = "~sq_c100_Main_Menu_frm~sq_ccboCREWID"
    # The independent renderer emits a bare leading-digit name outside our
    # current SQL spelling profile. Assert that exact observation, then compare
    # the adapter against an explicitly quoted expected source identity. This is
    # a separate regression, not another whole-renderer normalization claim.
    if oracle(args, args.sample, name) != "SELECT DISTINCTROW *\nFROM 101f_cbo_Crew_Id_qry;\n":
        raise RuntimeError("Observed leading-digit source spelling changed")
    expected = run(args.cli, "query", "parse", "SELECT DISTINCTROW * FROM [101f_cbo_Crew_Id_qry]")
    if reconstructed(args, args.sample, name) != expected:
        raise RuntimeError("Explicitly quoted saved source identity changed")
    if args.sample.read_bytes() != original:
        raise RuntimeError("Registered original application changed")
    print("13 independent reconstructions/family and a separate quoted-name example regression pass. No execution, native Access or application save/reopen qualified.")


if __name__ == "__main__":
    main()
