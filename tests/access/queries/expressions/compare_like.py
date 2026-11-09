"""Compare native-file LIKE projections/filters with independent text/regex models.

Jackcess supplies inert stored rows only. Python supplies the explicitly ordinal
expected matches; neither is a native Microsoft Access SQL execution oracle.
"""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
from compare_browsing import RECIPES, PROFILES, run

# Explicit independent regular expressions for each documented pattern pair.
CASES = [
    ("*", "%", r".*"), ("a*", "a%", r"a.*"), ("?", "_", r"."),
    ("??", "__", r".."), ("#", "[0-9]", r"[0-9]"),
    ("*#*", "%[0-9]%", r".*[0-9].*"),
    ("[a-c]*", "[a-c]%", r"[a-c].*"),
    ("[!a-c]*", "[^a-c]%", r"[^a-c].*"),
    ("[*]", "*", r"\*"), ("[?]", "?", r"\?"),
    ("[#]", "#", r"\#"), ("[[]]", "[[]]", r"\[\]"),
    ("[]", "[]", r""), ("[!-]", "[^-]", r"[^-]"),
    ("[a-cx-z]", "[a-cx-z]", r"[a-cx-z]"),
    ("é*", "é%", r"é.*"), ("%", "[%]", r"%"),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", required=True, type=Path)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, suffix in PROFILES:
            run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "LikeFixture.java", args.directory / f"Like-{profile}.{suffix}", profile)
    for profile, suffix in PROFILES:
        path = args.directory / f"Like-{profile}.{suffix}"
        before = path.read_bytes()
        assert before[20:24] == bytes([{"V2000": 1, "V2016": 5, "V2019": 6}[profile], 0, 0, 0])
        raw = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", path, "Text samples")
        values = []
        for line in raw.splitlines():
            _, ident, label = line.split("\t")
            values.append((int(ident.split(":")[1]), None if label == "N" else bytes.fromhex(label[1:]).decode("utf-8")))
        assert len(values) == 28
        values.sort()
        projections = "PARAMETERS P TEXT(120); SELECT ID, Label LIKE P AS Matched, Label NOT LIKE P AS Rejected FROM [Text samples] ORDER BY ID"
        filters = "PARAMETERS P TEXT(120); SELECT ID FROM [Text samples] WHERE Label LIKE P ORDER BY ID"
        checks = 0
        for mode_index, mode in enumerate(["ansi89", "ansi92"]):
            def command(sql, value):
                return [args.cli, "query", "run", sql, "--database", path, "--collation", "ordinal", "--like-mode", mode, "--param", value]
            for *patterns, regex in CASES:
                truths = [(ident, None if text is None else re.fullmatch(regex, text, flags=re.DOTALL) is not None) for ident, text in values]
                projected = "ID\tMatched\tRejected\n" + "".join(f"{ident}\t" + ("NULL\tNULL" if truth is None else "TRUE\tFALSE" if truth else "FALSE\tTRUE") + "\n" for ident, truth in truths)
                selected = "ID\n" + "".join(f"{ident}\n" for ident, truth in truths if truth)
                assert run(*command(projections, "P=text:" + patterns[mode_index])) == projected, (profile, mode, patterns)
                assert run(*command(filters, "P=text:" + patterns[mode_index])) == selected, (profile, mode, patterns)
                checks += 2
            unknown = "ID\tMatched\tRejected\n" + "".join(f"{ident}\tNULL\tNULL\n" for ident, _ in values)
            assert run(*command(projections, "P=null:")) == unknown
            checks += 1
            for invalid in ["[", "[z-a]", "[a-b-c]"]:
                failed = subprocess.run(list(map(str, command(filters, "P=text:" + invalid))), capture_output=True)
                assert failed.returncode == 2 and not failed.stdout
                checks += 1
        assert path.read_bytes() == before
        print(f"{profile}: {checks} ordinal projection/filter/error controls over 28 independent rows; unchanged SHA-256 {hashlib.sha256(before).hexdigest()}")


if __name__ == "__main__":
    main()
