"""Decode CLI TSV transport to escaped display cells for existing oracle checks.

Java/native engineering examples emit their own tagged records and bypass this
adapter. This removes CSV quoting only; it never unescapes display values.
"""
import csv
import io

MAX_CLI_OUTPUT_BYTES = 1_048_576


def display_tsv(text):
    # A maximum native Binary cell has 131,074 displayed characters, already
    # above Python csv's default field limit. Preserve the CLI's bounded envelope.
    previous = csv.field_size_limit()
    try:
        csv.field_size_limit(max(previous, MAX_CLI_OUTPUT_BYTES))
        return "".join("\t".join(row) + "\n" for row in csv.reader(io.StringIO(text), delimiter="\t", strict=True))
    finally:
        csv.field_size_limit(previous)


def command_output(arguments, text):
    args = tuple(map(str, arguments))
    if any(args[i:i + 2] == ("--format", "table") for i in range(len(args) - 1)) or "--format=table" in args:
        return text
    if len(args) > 1 and (args[1] in ("inspect", "tables", "rows") or args[1:3] in (("query", "eval"), ("query", "run"))):
        return display_tsv(text)
    return text
