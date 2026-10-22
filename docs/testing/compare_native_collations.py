"""Compare native per-field General routing with independent short-key models.

Original mixed-policy files are generated only on request into a fresh local
corpus. Jackcess supplies exact metadata and raw weights, not native Access SQL.
"""
import argparse
import base64
import os
from pathlib import Path
import re
import subprocess
import tempfile

from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
QUERIES = {
    "compare": "SELECT ID,LegacyText<ChrW(769) AS L,ModernText<ChrW(769) AS M,ChrW(128)<ChrW(769) AS D,StrComp(ChrW(128),ChrW(769)) AS ModuleMode,StrComp(ChrW(128),ChrW(769),2) AS DatabaseMode FROM PolicyProbe WHERE ID=1",
    "orderLegacyText": "SELECT ID FROM PolicyProbe ORDER BY LegacyText,ID",
    "orderModernText": "SELECT ID FROM PolicyProbe ORDER BY ModernText,ID",
    "filterLegacyText": "SELECT ID FROM PolicyProbe WHERE LegacyText<ChrW(769) ORDER BY ID",
    "filterModernText": "SELECT ID FROM PolicyProbe WHERE ModernText<ChrW(769) ORDER BY ID",
    "group": "SELECT Min(ID) AS ID,Count(*) AS N FROM PolicyProbe GROUP BY LegacyText ORDER BY Min(ID)",
    "like": "SELECT ModernText LIKE ('[' & ChrW(769) & '-' & ChrW(128) & ']') AS M FROM PolicyProbe WHERE ID=1",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    if digest(args.jar) != JAR_SHA256:
        raise ValueError("unexpected independent artifact")
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, suffix in PROFILES:
            run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                RECIPES / "NativePolicyFixture.java", args.directory / f"Policies-{profile}.{suffix}", profile)
    controls = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-native-policy-oracle-") as directory:
        run("javac", "-encoding", "UTF-8", "-cp", args.jar, "-d", directory,
            RECIPES / "NativePolicyOracle.java")
        classpath = os.pathsep.join([directory, str(args.jar)])
        for profile, suffix in PROFILES:
            path = args.directory / f"Policies-{profile}.{suffix}"
            before = digest(path)
            for oracle_policy, policy in [("database", "database"), ("0", "general-legacy"), ("1", "general-2010")]:
                independent = run("java", "-Dstdout.encoding=UTF-8", "-cp", classpath,
                                  "com.healthmarketscience.jackcess.impl.NativePolicyOracle", path, oracle_policy)
                for line in independent.splitlines():
                    case, encoded = line.split("\t")
                    expected = base64.b64decode(encoded, validate=True).decode("utf-8")
                    wrappers = [None, "Choose(1,{field})", "Switch(True,{field})",
                                "Choose(1,Switch(True,{field}),{field})"]
                    wrappers += [name + "('{field}','PolicyProbe','ID=' & CStr(ID))"
                                 for name in ["DLookup", "DFirst", "DLast", "DMin", "DMax"]]
                    for wrapper in wrappers:
                        sql = QUERIES[case]
                        if wrapper:
                            sql = re.sub(r"\b(?:LegacyText|ModernText)\b",
                                         lambda match: wrapper.format(field=match.group()), sql)
                        for options in [[], ["--index", "PolicyProbe", "ModernIndex"]]:
                            actual = run(args.cli, "query", "run", sql, "--database", path,
                                         "--collation", policy, "--like-mode", "ansi89",
                                         "--number-format", "en-us", *options)
                            if actual != expected:
                                raise ValueError(f"{profile} {policy} {case}: independent result differs, "
                                                 f"selected={bool(options)}, wrapper={wrapper}")
                            controls += 1
            failed = subprocess.run([str(args.cli), "query", "run", "SELECT LegacyText=ModernText FROM PolicyProbe",
                                     "--database", str(path), "--collation", "database"], capture_output=True, text=True, check=False)
            if failed.returncode != 2 or failed.stdout or failed.stderr != "error: conflicting text comparison policies\n":
                raise ValueError("conflicting native identities did not fail atomically")
            controls += 1
            legacy_pattern = subprocess.run([str(args.cli), "query", "run", QUERIES["like"],
                                            "--database", str(path), "--collation", "general-legacy",
                                            "--like-mode", "ansi89"], capture_output=True, text=True, check=False)
            if legacy_pattern.returncode != 2 or legacy_pattern.stdout or legacy_pattern.stderr != "error: invalid LIKE pattern\n":
                raise ValueError("legacy override failed to retain its pattern-range validation")
            controls += 1
            for selector in ["Choose(1,LegacyText,ModernText)",
                             "Switch(True,LegacyText,False,ModernText)"]:
                failed = subprocess.run(
                    [str(args.cli), "query", "run", f"SELECT {selector}=ChrW(128) FROM PolicyProbe WHERE ID=1",
                     "--database", str(path), "--collation", "database"],
                    capture_output=True, text=True, check=False,
                )
                if (failed.returncode != 2 or failed.stdout
                        or failed.stderr != "error: conflicting text comparison policies\n"):
                    raise ValueError("mixed selector policy did not fail atomically")
                controls += 1
            if digest(path) != before:
                raise ValueError("policy comparison changed the source")
            controls += 1
            print(f"{profile}: per-field/default/override query controls; unchanged {before}", flush=True)
    if digest(args.jar) != JAR_SHA256:
        raise ValueError("independent artifact changed")
    print(f"Passed {controls} native per-field policy model/source controls; native Windows comparisons deferred.")


if __name__ == "__main__":
    main()
