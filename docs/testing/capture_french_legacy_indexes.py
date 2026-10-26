#!/usr/bin/env python3
"""Capture exact stored French legacy keys without a TableCraft/ICU oracle."""

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess

from encoded_delete_originals import wire

ROOT = Path(__file__).resolve().parents[2]
HELPER = 'com.healthmarketscience.jackcess.impl.FrenchLegacyIndexFixture'
JAR_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
PROFILES = ('V2016', 'V2019')
TABLES = ('French', 'GeneralLegacy')
VALUES = [None, '', 'cote', 'coté', 'côte', 'côté', 'COTE', 'cote',
          'cote ', 'co-te', "co'te", 'co te', 'co.te', 'cote!',
          'é', 'e\u0301', 'è', 'ê', 'ë', 'e', 'E', 'É',
          'œ', 'oe', 'æ', 'ae', 'ß', 'ss', 'Œ', 'OE',
          '😀', 'a\x00b', 'a' * 255, 'a' * 254 + 'é',
          'a' * 254 + 'è', 'à' * 255, 'côté', 'cote  ']


def pin(path):
    raw = path.read_bytes()
    return {'path': path.resolve().relative_to(ROOT).as_posix(), 'bytes': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest()}


def invoke(command, output):
    result = subprocess.run(command, capture_output=True, text=True, timeout=120)
    record = {'command': command, 'exit_code': result.returncode,
              'stdout': result.stdout, 'stderr': result.stderr}
    with output.open('x') as stream:
        json.dump(record, stream, indent=2, ensure_ascii=True)
        stream.write('\n')
    assert result.returncode == 0, output
    return result.stdout


def parse(text):
    tables = {}
    for line in text.splitlines():
        fields = line.split('\t')
        kind = fields[0]
        if kind == 'FILE':
            continue
        name = fields[1]
        if kind == 'TABLE':
            assert name not in tables
            tables[name] = {'rows': [], 'indexes': {}, 'sort': None,
                            'count': int(fields[2]), 'definition': int(fields[3])}
            continue
        table = tables[name]
        if kind == 'SORT':
            assert table['sort'] is None
            table['sort'] = {'lcid': int(fields[2]), 'variant': int(fields[3]),
                             'version': int(fields[4]), 'offset': int(fields[5]),
                             'raw': fields[6]}
        elif kind == 'ROW':
            encoded = fields[5]
            value = None if encoded == 'NULL' else base64.b64decode(encoded.removeprefix('UTF8:')).decode('utf8')
            table['rows'].append({'id': int(fields[2]), 'page': int(fields[3]),
                                  'slot': int(fields[4]), 'value': value})
        elif kind == 'INDEX':
            index = fields[2]
            assert index not in table['indexes']
            table['indexes'][index] = {'physical': int(fields[3]), 'root': int(fields[4]),
                                       'count': int(fields[5]), 'owned_pages': int(fields[6]),
                                       'order': [], 'keys': []}
        elif kind == 'ORDER':
            table['indexes'][fields[2]]['order'].append(int(fields[3]))
        elif kind == 'KEY':
            table['indexes'][fields[2]]['keys'].append((fields[5], int(fields[3]), int(fields[4])))
        else:
            raise AssertionError(kind)
    assert set(tables) == set(TABLES)
    return tables


def verify(raw, tables):
    for name in TABLES:
        table = tables[name]
        assert table['count'] == len(VALUES)
        assert [row['id'] for row in table['rows']] == list(range(1, len(VALUES) + 1))
        assert [row['value'] for row in table['rows']] == VALUES
        sort = table['sort']
        lcid = 1036 if name == 'French' else 1033
        assert (sort['lcid'], sort['variant'], sort['version']) == (lcid, 0, 0)
        expected_descriptor = lcid.to_bytes(2, 'little') + bytes([0, 0])
        assert bytes.fromhex(sort['raw']) == expected_descriptor
        assert raw[sort['offset']:sort['offset'] + len(expected_descriptor)] == expected_descriptor
        assert set(table['indexes']) == {'Asc', 'Twin', 'Desc'}
        assert table['indexes']['Asc'] == table['indexes']['Twin']
        locations = {(row['page'], row['slot']): row['id'] for row in table['rows']}
        for index in table['indexes'].values():
            entries, leaves, nodes = wire.decode_tree(raw, index['root'], table['definition'])
            assert entries == index['keys']
            assert not nodes and len(leaves) == index['owned_pages'] == 1
            assert index['count'] == len(entries) == len(VALUES)
            assert index['order'] == [locations[(page, slot)] for _, page, slot in entries]
            assert set(index['order']) == set(range(1, len(VALUES) + 1))
        quartet = [identifier for identifier in table['indexes']['Asc']['order']
                   if identifier in (3, 4, 5, 6)]
        assert quartet == ([3, 5, 4, 6] if name == 'French' else [3, 4, 5, 6])
        descending = [identifier for identifier in table['indexes']['Desc']['order']
                      if identifier in (3, 4, 5, 6)]
        assert descending == list(reversed(quartet))
        assert any(len(bytes.fromhex(key)) == 510 for key, _, _ in table['indexes']['Asc']['keys'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--classes', type=Path, required=True)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--existing-original', action='append', choices=PROFILES, default=[])
    args = parser.parse_args()
    assert pin(args.jar)['sha256'] == JAR_SHA256
    assert not args.report.exists()
    args.directory.mkdir(parents=True, exist_ok=True)
    command = ['java', '-cp', os.pathsep.join(map(str, (args.jar, args.classes))), HELPER]
    report = {'schema_version': 1, 'scope': 'Pinned Jackcess stored-index oracle; no TableCraft, SQL comparison, ICU or native NLS qualification',
              'artifacts': [pin(Path(__file__)), pin(Path('docs/testing/FrenchLegacyIndexFixture.java')),
                            pin(args.jar), pin(args.classes / Path(*HELPER.split('.')).with_suffix('.class')),
                            pin(Path(wire.__file__))], 'profiles': []}
    for profile in PROFILES:
        original = args.directory / f'French-{profile}.accdb'
        if profile not in args.existing_original:
            invoke(command + ['generate', str(original), profile], args.directory / f'{profile}-generate.json')
        before = pin(original)
        stdout = invoke(command + ['oracle', str(original)], args.directory / f'{profile}-getter.json')
        tables = parse(stdout)
        verify(original.read_bytes(), tables)
        assert pin(original) == before
        report['profiles'].append({'profile': profile, 'original': before, 'tables': tables})
    report['artifacts'].extend(pin(path) for path in sorted(args.directory.glob('*.json')))
    report['disagreements'] = 0
    with args.report.open('x') as stream:
        json.dump(report, stream, indent=2, ensure_ascii=True)
        stream.write('\n')
    print(f'{len(report["profiles"])} profiles passed exact descriptors, raw keys and order: {args.report}')


if __name__ == '__main__':
    main()
