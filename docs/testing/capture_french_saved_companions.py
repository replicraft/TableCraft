#!/usr/bin/env python3
"""Create inert saved-SELECT companions and prove original field/page preservation."""

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess

from capture_french_comparison import parse as field_model

ROOT = Path(__file__).resolve().parents[2]
JAR_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
HELPER = 'com.healthmarketscience.jackcess.impl.FrenchSavedCompanion'
FIELD_HELPER = 'com.healthmarketscience.jackcess.impl.FrenchComparisonFixture'
EXPECTED_SQL = 'SELECT [src].[ID], [src].[FrenchShort], [src].[FrenchMemo]\nFROM Comparisons AS src;'
PAGE_BYTES = 4096
EXPECTED_ATTRIBUTES = ['0', '1', '3', '5', '6', '6', '6', '-1']


def pin(path):
    raw = path.read_bytes()
    return {'path': path.resolve().relative_to(ROOT).as_posix(), 'bytes': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest()}


def invoke(command, output):
    result = subprocess.run(command, capture_output=True, text=True, timeout=120)
    with output.open('x') as stream:
        json.dump({'command': command, 'exit_code': result.returncode,
                   'stdout': result.stdout, 'stderr': result.stderr}, stream, indent=2)
        stream.write('\n')
    assert result.returncode == 0 and not result.stderr, output
    return result.stdout


def saved_model(text):
    result = {'queries': [], 'objects': [], 'fragments': [], 'properties': [], 'protected': {}}
    for line in text.splitlines():
        fields = line.split('\t')
        kind = fields[0]
        if kind == 'FILE':
            continue
        if kind == 'QUERY':
            result['queries'].append({'name': fields[1], 'type': fields[2],
                                      'object_id': int(fields[3]), 'flags': int(fields[4]),
                                      'owner_access': base64.b64decode(fields[5]).decode(),
                                      'sql': base64.b64decode(fields[6]).decode()})
        elif kind == 'QOBJECT':
            result['objects'].append(list(map(int, fields[1:])))
        elif kind == 'FRAGMENT':
            result['fragments'].append(fields[1:])
        elif kind == 'QPROPERTY':
            result['properties'].append(fields[1:])
        elif kind == 'PROTECTED':
            page = int(fields[1])
            assert page not in result['protected']
            result['protected'][page] = {'role': fields[2], 'sha256': fields[3]}
        else:
            raise AssertionError(kind)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('fields', 'jar', 'classes', 'field-classes', 'directory', 'report'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--existing-companion', action='append', choices=('V2016', 'V2019'), default=[])
    args = parser.parse_args()
    assert pin(args.jar)['sha256'] == JAR_SHA256
    assert not args.report.exists()
    args.directory.mkdir(parents=True, exist_ok=True)
    fields = json.loads(args.fields.read_text())
    command = ['java', '-cp', os.pathsep.join(map(str, (args.jar, args.classes))), HELPER]
    field_command = ['java', '-cp', os.pathsep.join(map(str, (args.jar, args.field_classes))), FIELD_HELPER]
    report = {'scope': 'Saved SELECT metadata construction/getter and exact protected field/page preservation only; no SQL execution',
              'artifacts': [pin(Path(__file__)), pin(Path('docs/testing/FrenchSavedCompanion.java')),
                            pin(args.fields), pin(args.jar),
                            pin(args.classes / Path(*HELPER.split('.')).with_suffix('.class'))],
              'profiles': []}
    for entry in fields['profiles']:
        profile = entry['profile']
        source = ROOT / entry['original']['path']
        assert pin(source) == entry['original']
        getter = ROOT / entry['getter']['path']
        assert pin(getter) == entry['getter']
        before = field_model(json.loads(getter.read_text())['stdout'])
        source_pin = pin(source)
        original_saved = saved_model(invoke(command + ['oracle', str(source)],
                                           args.directory / f'{profile}-source-protected.json'))
        assert original_saved['queries'] == original_saved['objects'] == original_saved['fragments'] == []
        target = args.directory / f'Named-{profile}.accdb'
        if profile not in args.existing_companion:
            invoke(command + ['copy', str(source), str(target)], args.directory / f'{profile}-create.json')
        target_pin = pin(target)
        actual = saved_model(invoke(command + ['oracle', str(target)],
                                   args.directory / f'{profile}-saved-getter.json'))
        assert len(actual['queries']) == len(actual['objects']) == 1
        query = actual['queries'][0]
        assert query['name'] == 'FrenchFields' and query['type'] == 'SELECT'
        assert query['flags'] == 0 and query['owner_access'] == '' and query['sql'] == EXPECTED_SQL
        assert actual['objects'][0][0] == query['object_id']
        assert actual['objects'][0][2:] == [5, 0]
        assert actual['properties'] == []
        assert [row[0] for row in actual['fragments']] == EXPECTED_ATTRIBUTES
        assert len(actual['fragments']) == 8
        after = field_model(invoke(field_command + ['oracle', str(target)],
                                  args.directory / f'{profile}-field-getter.json'))
        assert after == before
        source_raw, target_raw = source.read_bytes(), target.read_bytes()
        protected = []
        for page, proof in original_saved['protected'].items():
            start, end = page * PAGE_BYTES, (page + 1) * PAGE_BYTES
            assert hashlib.sha256(source_raw[start:end]).hexdigest() == proof['sha256']
            assert target_raw[start:end] == source_raw[start:end]
            protected.append({'page': page, **proof})
        assert pin(source) == source_pin and pin(target) == target_pin
        changed = [page for page in range(len(source_raw) // PAGE_BYTES)
                   if source_raw[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
                   != target_raw[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]]
        assert not set(changed).intersection(original_saved['protected'])
        report['profiles'].append({'profile': profile, 'source': source_pin, 'companion': target_pin,
                                   'saved': {key: value for key, value in actual.items() if key != 'protected'},
                                   'protected_original_pages': protected,
                                   'changed_existing_pages': changed,
                                   'additional_pages': (len(target_raw) - len(source_raw)) // PAGE_BYTES,
                                   'field_rows_unchanged': len(before['rows']),
                                   'field_values_unchanged': len(before['values']) * 4})
    report['artifacts'].extend(pin(path) for path in sorted(args.directory.glob('*.json')))
    report['counts'] = {'new_companions': 2, 'saved_selects': 2, 'rows_unchanged': 76,
                        'text_memo_values_unchanged': 304,
                        'protected_original_pages_unchanged': sum(len(item['protected_original_pages']) for item in report['profiles'])}
    with args.report.open('x') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    print(json.dumps(report['counts']))


if __name__ == '__main__':
    main()
