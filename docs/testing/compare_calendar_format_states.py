"""Independent named date formatting/calendar state controls; Access unrun."""

import argparse
import csv
import datetime
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, display
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_union_types import JAR_SHA256

MONTHS = {
    "en-US": ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
    "de-DE": ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
}
MONTH_ABBREVIATIONS = {
    "en-US": ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
    "de-DE": ["Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez"],
}
DAYS = {
    "en-US": ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
    "de-DE": ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"],
}
DAY_ABBREVIATIONS = {"en-US": ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"], "de-DE": ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"]}


def text(value):
    return display("T" + value.encode().hex())


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256, "Unreviewed reference artifact"
    controls = 0

    def query(mode, sql, expected=None, options=(), diagnostic=None, profile="iso"):
        nonlocal controls
        result = subprocess.run(
            [str(args.cli), "query", mode, sql, "--date-format", profile, *map(str, options)],
            capture_output=True, text=True, encoding="utf-8",
        )
        if diagnostic:
            assert result.returncode == 2 and not result.stdout, (sql, "non-atomic refusal")
            assert diagnostic in result.stderr, (sql, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[1:] == expected, (sql, rows[1:], expected)
        controls += 1

    for mode in ["eval", "run"]:
        for code in [0, 1, 11, 65535]:
            error = f"CVErr({code})"
            for expression, diagnostic in [
                (f"FormatDateTime({error},4)", "type mismatch"),
                (f"FormatDateTime(NULL,{error})", "type mismatch"),
                (f"FormatDateTime({error},5)", "domain"),
                (f"MonthName({error})", "type mismatch"),
                (f"MonthName(0,{error})", "type mismatch"),
                (f"WeekdayName({error},False,{error})", "type mismatch"),
                (f"WeekdayName(0,False,{error})", "domain"),
            ]:
                query(mode, f"SELECT 1,{expression}", diagnostic=diagnostic)
            date = datetime.date(1899, 12, 30) + datetime.timedelta(days=code)
            query(mode, f"SELECT FormatDateTime(CLng({error}),2)", [[text(date.isoformat())]])
            month = "Jan" if code else "January"
            day = "Mon" if code else "Monday"
            query(mode, f"SELECT MonthName(1,CBool(CLng({error})))", [[text(month)]])
            query(mode, f"SELECT WeekdayName(1,CBool(CLng({error})),2)", [[text(day)]])
        for expression in ["FormatDateTime(CVErr(11),1/0)", "MonthName(CVErr(11),1/0)", "WeekdayName(0,False,1/0)"]:
            query(mode, f"SELECT {expression}", diagnostic="division by zero")
        for profile in MONTHS:
            for abbreviated in [False, True]:
                months = MONTH_ABBREVIATIONS[profile] if abbreviated else MONTHS[profile]
                days = DAY_ABBREVIATIONS[profile] if abbreviated else DAYS[profile]
                for ordinal, name in enumerate(months, start=1):
                    query(mode, f"SELECT MonthName({ordinal},{str(abbreviated)})", [[text(name)]], profile=profile)
                for ordinal, name in enumerate(days, start=1):
                    query(mode, f"SELECT WeekdayName({ordinal},{str(abbreviated)},1)", [[text(name)]], profile=profile)

    for suffix, original_digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        before = registered_input_hash(path, original_digest)
        assert digest(path) == before
        reference = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows",
        ], text=True, encoding="utf-8")
        identities = [int(line.split("\t")[1].removeprefix("I32:")) for line in reference.splitlines()]
        assert len(identities) == len(set(identities)) == 20_000 and 1 in identities
        options = ["--database", path, "--native-bytes", "67108864", "--native-rows", "100000", "--native-steps", "10000000", "--execution-steps", "10000000"]
        for index in [[], ["--index", "IndexedRows", "IdAsc"]]:
            chosen = [*options, *index]
            for expression, expected in [
                ("FormatDateTime(CLng(CVErr(ID)),2)", text("1899-12-31")),
                ("MonthName(CLng(CVErr(ID)))", text("January")),
                ("WeekdayName(1,CBool(CLng(CVErr(ID))),2)", text("Mon")),
            ]:
                query("run", f"SELECT {expression} FROM IndexedRows WHERE ID=1", [[expected]], chosen)
            for expression, diagnostic in [
                ("FormatDateTime(CVErr(ID),5)", "domain"),
                ("FormatDateTime(NULL,CVErr(ID))", "type mismatch"),
                ("MonthName(0,CVErr(ID))", "type mismatch"),
                ("WeekdayName(0,False,CVErr(ID))", "domain"),
                ("MonthName(CVErr(ID),1/(ID-1))", "division by zero"),
            ]:
                query("run", f"SELECT 1,{expression} FROM IndexedRows WHERE ID=1", options=chosen, diagnostic=diagnostic)
        assert digest(path) == before, "Input changed"
        print(f"{path.name}: unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} calendar/date-format state controls; native Access unrun.")


if __name__ == "__main__":
    main()
