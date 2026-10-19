"""Independent display checks for typed stored collection CLI reports."""
import csv
import io
import subprocess

from compare_browsing import display
from compare_cached_pages import date_display


def report(cli, path, table, flag, column, kind, parents, expected, headers):
    output = subprocess.check_output(list(map(str, [cli, 'inspect', path, '--table', table, flag, column, '--format', 'tsv'])), text=True, encoding='utf-8')
    rows = list(csv.reader(io.StringIO(output), delimiter='\t', strict=True))
    prefix = [['Table', table], ['Column', column], ['Parent rows', str(len(parents))], [kind, str(len(expected))], ['Parent ordinal', 'Complex key', kind]]
    assert rows == prefix + parents + [headers] + expected, 'CLI collection cells, membership, stored order or exact DATE bits differ'


def time_bits(tag):
    return 'NULL' if tag == 'N' else '0x' + tag.removeprefix('Y64:')


def attachment_report(args, path, tagged):
    expected = []
    for row in (line.split('\t') for line in tagged.splitlines()):
        parent, identity, url, name, kind, data, modified, flags = row
        expected.append([parent, identity, display(url), display(name), display(kind),
                         'NULL' if data == 'N' else str(len(bytes.fromhex(data[1:]))),
                         date_display(modified), time_bits(modified), display(flags)])
    # Original recipe has one populated parent and one independently assigned
    # empty parent; nullable payload mutations leave their keys unchanged.
    report(args.cli, path, 'Collections', '--attachments', 'Files', 'Stored attachments',
           [['0', '1', '6'], ['1', '2', '0']], expected,
           ['Parent ordinal', 'Identity', 'File URL', 'File name', 'File type', 'Encoded bytes', 'Timestamp', 'Timestamp bits', 'Flags'])


def history_report(args, path, table, memo, tagged):
    parents, expected = [], []
    for line in tagged.splitlines():
        cells = line.split('\t')
        _, parent, key = cells[:3]
        assert (len(cells) - 3) % 3 == 0
        parents.append([parent, 'NULL' if key == 'N' else key, str((len(cells) - 3) // 3)])
        for i in range(3, len(cells), 3):
            identity, text, modified = cells[i:i + 3]
            expected.append([parent, display(identity), display(text), date_display(modified), time_bits(modified)])
    report(args.cli, path, table, '--version-history', memo, 'Stored versions', parents, expected,
           ['Parent ordinal', 'Identity', 'Value', 'Modified', 'Modified bits'])


def refused(cli, path, table, flag, column):
    result = subprocess.run(list(map(str, [cli, 'inspect', path, '--table', table, flag, column])), capture_output=True)
    assert result.returncode == 2 and not result.stdout, 'Unqualified collection published CLI success output'
