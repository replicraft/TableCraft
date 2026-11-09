#!/usr/bin/env python3
"""Capture French comparison field data; never invoke SQL or a comparator."""

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
JAR_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
HELPER = 'com.healthmarketscience.jackcess.impl.FrenchComparisonFixture'
PROFILES = ('V2016', 'V2019')
FIELDS = ('FrenchShort', 'FrenchMemo', 'GeneralShort', 'GeneralMemo')
BASE = (None, '', 'cote', 'coté', 'côte', 'côté', 'COTE', 'é', 'e\u0301', 'œ',
        'oe', 'æ', 'ae', 'ß', 'ss', 'a-b', "a'b", 'a b', 'a.b', '😀')


def cases():
    result = [(value, value) for value in BASE]
    for length in (256, 1024, 20000):
        result.extend([
            ('long-primary', 'e' * (length - 1) + 'a'),
            ('long-primary', 'e' * (length - 1) + 'b'),
            ('long-accent', 'é' + 'e' * (length - 1)),
            ('long-accent', 'e' * (length - 1) + 'é'),
            ('long-accent', 'è' + 'e' * (length - 2) + 'é'),
            ('long-accent', 'é' + 'e' * (length - 2) + 'è'),
        ])
    return result


def pin(path):
    raw = path.read_bytes()
    return {'path': path.resolve().relative_to(ROOT).as_posix(), 'bytes': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest()}


def invoke(command, output):
    result = subprocess.run(command, capture_output=True, text=True, timeout=120)
    record = {'command': command, 'exit_code': result.returncode,
              'stdout': result.stdout, 'stderr': result.stderr}
    with output.open('x') as stream:
        json.dump(record, stream, indent=2)
        stream.write('\n')
    assert result.returncode == 0, output
    return result.stdout


def parse(text):
    result = {'columns': {}, 'sort': {}, 'rows': {}, 'values': {},
              'properties': [], 'indexes': {}}
    for line in text.splitlines():
        cells = line.split('\t')
        kind = cells[0]
        if kind == 'FILE':
            continue
        if kind == 'TABLE':
            assert 'table' not in result
            result['table'] = cells[1]
            result['count'] = int(cells[2])
            result['definition_page'] = int(cells[3])
        elif kind == 'COLUMN':
            assert cells[1] not in result['columns']
            result['columns'][cells[1]] = {'type': cells[2], 'native_type': int(cells[3]),
                                          'storage': int(cells[4]), 'flags': int(cells[5]),
                                          'extended_flags': int(cells[6]), 'autonumber': cells[7]}
        elif kind == 'SORT':
            assert cells[1] not in result['sort']
            result['sort'][cells[1]] = {'lcid': int(cells[2]), 'variant': int(cells[3]),
                                       'version': int(cells[4]), 'offset': int(cells[5]),
                                       'raw': cells[6]}
        elif kind == 'PROPERTY':
            result['properties'].append(cells[1:])
        elif kind == 'ROW':
            identifier = int(cells[1])
            assert identifier not in result['rows']
            result['rows'][identifier] = list(map(int, cells[2:]))
            result['values'][identifier] = {}
        elif kind == 'VALUE':
            identifier = int(cells[1])
            field = cells[2]
            assert field not in result['values'][identifier]
            raw = base64.b64decode(cells[6], validate=True)
            assert hashlib.sha256(raw).hexdigest() == cells[5]
            assert len(raw) == int(cells[4]) * 2
            assert cells[3] in ('NULL', 'TEXT')
            if cells[3] == 'NULL':
                assert not raw
            result['values'][identifier][field] = None if cells[3] == 'NULL' else raw
        elif kind == 'INDEX':
            assert cells[1] not in result['indexes']
            result['indexes'][cells[1]] = {'physical': int(cells[2]), 'root': int(cells[3]),
                                          'entries': int(cells[4]), 'owned_pages': int(cells[5]),
                                          'order': []}
        elif kind == 'ORDER':
            result['indexes'][cells[1]]['order'].append(int(cells[2]))
        else:
            raise AssertionError(kind)
    return result


def validate(image, model):
    assert model['table'] == 'Comparisons' and model['count'] == 38
    assert model['properties'] == []
    assert set(model['columns']) == {'ID', *FIELDS}
    assert set(model['sort']) == set(FIELDS)
    assert list(model['rows']) == list(range(1, 39))
    assert model['columns']['ID'] == {'type': 'LONG', 'native_type': 4, 'storage': 4,
                                      'flags': 3, 'extended_flags': 0, 'autonumber': 'false'}
    for name in FIELDS:
        short = name.endswith('Short')
        assert model['columns'][name] == {'type': 'TEXT' if short else 'MEMO',
                                          'native_type': 10 if short else 12,
                                          'storage': 510 if short else 0,
                                          'flags': 2, 'extended_flags': 0, 'autonumber': 'false'}
        sort = model['sort'][name]
        lcid = 1036 if name.startswith('French') else 1033
        assert (sort['lcid'], sort['variant'], sort['version']) == (lcid, 0, 0)
        descriptor = lcid.to_bytes(2, 'little') + bytes([0, 0])
        assert bytes.fromhex(sort['raw']) == image[sort['offset']:sort['offset'] + 4] == descriptor
    assert set(model['indexes']) == {'Primary', 'FrenchShortAsc'}
    assert model['indexes']['Primary']['physical'] != model['indexes']['FrenchShortAsc']['physical']
    for index in model['indexes'].values():
        assert index['entries'] == 38 and index['owned_pages'] == 1
        assert sorted(index['order']) == list(range(1, 39))
    assert model['indexes']['Primary']['order'] == list(range(1, 39))
    differences = []
    actual_cells = []
    for identifier, pair in enumerate(cases(), 1):
        values = model['values'][identifier]
        assert set(values) == set(FIELDS)
        assert values['FrenchShort'] == values['GeneralShort']
        assert values['FrenchMemo'] == values['GeneralMemo']
        for field in FIELDS:
            expected = pair[0] if field.endswith('Short') else pair[1]
            expected_raw = None if expected is None else expected.encode('utf-16-le')
            actual = values[field]
            if actual != expected_raw:
                differences.append({'id': identifier, 'field': field,
                                    'input_sha256': None if expected_raw is None else hashlib.sha256(expected_raw).hexdigest(),
                                    'actual_sha256': None if actual is None else hashlib.sha256(actual).hexdigest()})
            actual_cells.append({'id': identifier, 'field': field, 'is_null': actual is None,
                                 'utf16_units': 0 if actual is None else len(actual) // 2,
                                 'sha256': None if actual is None else hashlib.sha256(actual).hexdigest()})
    return differences, actual_cells


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--classes', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--existing-original', action='append', choices=PROFILES, default=[])
    args = parser.parse_args()
    assert pin(args.jar)['sha256'] == JAR_SHA256
    assert not args.report.exists()
    args.directory.mkdir(parents=True, exist_ok=True)
    command = ['java', '-cp', os.pathsep.join(map(str, (args.jar, args.classes))), HELPER]
    report = {'scope': 'Actual native field-data observations only; no SQL/comparator/native semantics qualification',
              'artifacts': [pin(Path(__file__)), pin(Path('docs/testing/FrenchComparisonFixture.java')),
                            pin(args.jar), pin(args.classes / Path(*HELPER.split('.')).with_suffix('.class'))],
              'profiles': []}
    for profile in PROFILES:
        original = args.directory / f'Comparison-{profile}.accdb'
        if profile not in args.existing_original:
            invoke(command + ['generate', str(original), profile], args.directory / f'{profile}-generate.json')
        before = pin(original)
        getter = args.directory / f'{profile}-getter.json'
        text = invoke(command + ['oracle', str(original)], getter)
        model = parse(text)
        differences, cells = validate(original.read_bytes(), model)
        assert pin(original) == before
        metadata = {key: value for key, value in model.items() if key != 'values'}
        report['profiles'].append({'profile': profile, 'original': before, 'getter': pin(getter),
                                   'metadata': metadata, 'actual_cell_summaries': cells,
                                   'input_storage_differences': differences})
    report['artifacts'].extend(pin(path) for path in sorted(args.directory.glob('*.json')))
    report['counts'] = {'originals': 2, 'rows': 76, 'text_memo_cells': 304,
                        'input_storage_differences': sum(len(item['input_storage_differences']) for item in report['profiles'])}
    with args.report.open('x') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    print(json.dumps(report['counts']))


if __name__ == '__main__':
    main()
