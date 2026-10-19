"""Compare inert mixed collection pages against independent original Java getters."""
import argparse
import csv
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import display, escaped
from compare_cached_pages import date_display


def time_cell(tag):
    return 'NULL' if tag == 'N' else date_display(tag) + ' (bits=0x' + tag.removeprefix('Y64:') + ')'


def cell(kind, values):
    if kind in ('S', 'C'):
        return display(values[1])
    if values[1] == 'N':
        assert len(values) == 2
        return 'NULL'
    tags, items = values[2:], []
    if kind == 'M':
        items = list(map(display, tags))
    elif kind == 'A':
        assert len(tags) % 7 == 0
        for i in range(0, len(tags), 7):
            identity, url, name, label, length, timestamp, flags = tags[i:i + 7]
            length = 'NULL' if length == 'N' else length.removeprefix('L:')
            items.append('{id: ' + display(identity) + ', url: ' + display(url) + ', name: ' + display(name) + ', type: ' + display(label) + ', encoded_bytes: ' + length + ', timestamp: ' + time_cell(timestamp) + ', flags: ' + display(flags) + '}')
    else:
        assert kind.startswith('H:') and len(tags) % 3 == 0
        for i in range(0, len(tags), 3):
            identity, text, timestamp = tags[i:i + 3]
            items.append('{id: ' + display(identity) + ', value: ' + display(text) + ', modified: ' + time_cell(timestamp) + '}')
    return '[' + ', '.join(items) + ']'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('jar', 'cli', 'complex2007', 'complex2016', 'complex2019', 'attachment2007', 'attachment2016', 'attachment2019', 'history2007', 'history2016', 'history2019'):
        parser.add_argument('--' + name, required=True, type=Path)
    args = parser.parse_args()
    controls = 0
    with tempfile.TemporaryDirectory(prefix='tablecraft-collection-pages-') as directory:
        subprocess.run(['javac', '-cp', str(args.jar), '-d', directory, 'docs/testing/MultivalueOracle.java', 'docs/testing/CollectionPageOracle.java'], check=True)
        classpath = str(args.jar) + os.pathsep + directory
        for family in ('2007', '2016', '2019'):
            for recipe, tables in [('complex', ['Collections']), ('attachment', ['Collections']), ('history', ['Histories', 'OtherHistories'])]:
                path = getattr(args, recipe + family)
                before = path.read_bytes()
                for table in tables:
                    tagged = subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', classpath, 'CollectionPageOracle', str(path), table], text=True, encoding='utf-8')
                    declarations, values = [], {}
                    for line in tagged.splitlines():
                        parts = line.split('\t')
                        if parts[0] == 'H': declarations.append(parts[1:])
                        else: values[int(parts[0]), parts[1]] = parts[2:]
                    count = 1 + max(ordinal for ordinal, _ in values)
                    headers = []
                    for name, kind in declarations:
                        suffix = {'M': ' [multivalue]', 'A': ' [attachments]', 'C': ' [cached]'}.get(kind, '')
                        if kind.startswith('H:'): suffix = ' [history: ' + escaped(kind[2:]) + ']'
                        headers.append(escaped(name) + suffix)
                    expected = [[cell(kind, values[i, name]) for name, kind in declarations] for i in range(count)]
                    for offset, limit in [(0, 100), (1, 1), (2, 2), (count, 1), (4_294_967_295, 1)]:
                        command = list(map(str, [args.cli, 'rows', path, table, '--collections', '--offset', offset, '--limit', limit, '--format', 'tsv']))
                        output = subprocess.check_output(command, text=True, encoding='utf-8')
                        actual = list(csv.reader(io.StringIO(output), delimiter='\t', strict=True))
                        assert actual == [headers] + expected[offset:offset + limit], (family, table, offset)
                        controls += 1
                    pretty = subprocess.check_output(list(map(str, [args.cli, 'rows', path, table, '--collections', '--limit', 1, '--format', 'table'])), text=True, encoding='utf-8')
                    assert pretty.startswith('┌') and ('[attachments]' in pretty or '[history:' in pretty)
                    controls += 1
                assert path.read_bytes() == before
                print(family, recipe, hashlib.sha256(before).hexdigest(), 'unchanged', flush=True)
    print(controls, 'mixed collection page/offset/empty/pretty controls passed; native Access unrun')


if __name__ == '__main__':
    main()
