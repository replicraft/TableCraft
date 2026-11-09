#!/usr/bin/env python3
"""Capture public-writer AutoNumber DELETE facts; importing performs no I/O."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

JAR_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
HELPER = 'com.healthmarketscience.jackcess.impl.AutoNumberDeleteFixture'
CLASS_FILE = Path(*HELPER.split('.')).with_suffix('.class')
PROFILES = ('V2016', 'V2019')
TABLES = ('Numbered', 'Loose')
KEPT = (1, 3, 7, 64, 128, 259, 260)
SELECTIONS = {'first': {1}, 'highest': {260}, 'subset': {3, 128, 260}, 'all': set(KEPT)}
APPEND_SCORE = -1234567
REPOSITORY = Path(__file__).resolve().parents[2]


def pin(path):
    raw = path.read_bytes()
    return {'path': path.resolve().relative_to(REPOSITORY).as_posix(),
            'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}


def invoke(command, log):
    result = subprocess.run(command, text=True, capture_output=True, timeout=120)
    record = {'command': command, 'returncode': result.returncode,
              'stdout': result.stdout, 'stderr': result.stderr}
    with log.open('x') as output:
        json.dump(record, output, indent=2)
        output.write('\n')
    if result.returncode:
        raise RuntimeError(f'Public helper failed; retained {log}')
    return result.stdout


def parse(stdout):
    result = {name: {'rows': [], 'columns': {}, 'indexes': {}, 'order': {},
                     'properties': []} for name in TABLES}
    for line in stdout.splitlines():
        fields = line.split('\t')
        kind = fields[0]
        if kind == 'FILE':
            continue
        table = result[fields[1]]
        if kind == 'TABLE':
            values = list(map(int, fields[2:]))
            (table['row_count'], table['definition_page'], table['raw_row_count'],
             table['long_counter'], table['long_offset'], table['complex_counter'],
             table['complex_offset'], table['page_size']) = values
        elif kind == 'COLUMN':
            table['columns'][fields[2]] = {'type': fields[3], 'autonumber': fields[4],
                                          'offset': int(fields[5]), 'flags': int(fields[6]),
                                          'extended_flags': int(fields[7])}
        elif kind == 'PROPERTY':
            table['properties'].append(fields[2:])
        elif kind == 'ROW':
            table['rows'].append(list(map(int, fields[2:])))
        elif kind == 'INDEX':
            table['indexes'][fields[2]] = list(map(int, fields[3:]))
        elif kind == 'ORDER':
            table['order'].setdefault(fields[2], []).append(int(fields[3]))
        else:
            raise AssertionError(f'Unknown getter record {kind}')
    return result


def validate(model, removed_table=None, removed=frozenset(), appended_table=None):
    for name in TABLES:
        table = model[name]
        expected = {key: key * 10 for key in KEPT
                    if name != removed_table or key not in removed}
        if name == appended_table:
            expected[261] = APPEND_SCORE
        actual = {row[0]: row[1] for row in table['rows']}
        assert len(actual) == len(table['rows']) and actual == expected
        assert table['row_count'] == table['raw_row_count'] == len(expected)
        assert table['long_counter'] == (261 if name == appended_table else 260)
        assert table['complex_counter'] == 0
        assert table['columns']['ID']['autonumber'] == 'true'
        assert table['columns']['ID']['flags'] == 7
        assert table['columns']['Score']['flags'] == 3
        assert table['properties'] == []
        if name == 'Numbered':
            indexes = table['indexes']
            assert set(indexes) == {'Primary', 'PrimaryAlias'}
            assert indexes['Primary'] == indexes['PrimaryAlias']
            assert indexes['Primary'][0] == 0
            assert indexes['Primary'][2] == len(expected)
            for order in table['order'].values():
                assert order == sorted(expected)
        else:
            assert table['indexes'] == {} and table['order'] == {}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--classes', type=Path, required=True)
    parser.add_argument('--existing-original', action='append', choices=PROFILES, default=[])
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    assert pin(args.jar)['sha256'] == JAR_SHA256
    assert not args.report.exists()
    args.directory.mkdir(parents=True, exist_ok=True)
    command = ['java', '-cp', os.pathsep.join(map(str, (args.jar, args.classes))), HELPER]
    report = {'schema_version': 1,
              'scope': 'Repository-owned ordinary LONG AutoNumber public-writer facts, not native Access qualification',
              'construction': {'generated_rows_per_table': 260, 'kept_ids': list(KEPT),
                               'deletes_are_part_of_original_construction': True},
              'artifacts': [pin(Path(__file__)), pin(Path('docs/testing/AutoNumberDeleteFixture.java')),
                            pin(args.jar), pin(args.classes / CLASS_FILE)],
              'observations': []}
    originals = []
    for profile in PROFILES:
        original = args.directory / f'AutoNumber-{profile}-before.accdb'
        if profile not in args.existing_original:
            invoke(command + ['generate', str(original), profile],
                   args.directory / f'{profile}-generate.json')
        before = pin(original)
        originals.append(before)
        stdout = invoke(command + ['oracle', str(original)],
                        args.directory / f'{profile}-before-getter.json')
        baseline = parse(stdout)
        validate(baseline)
        report['observations'].append({'profile': profile, 'state': 'before',
                                       'artifact': before, 'tables': baseline})
        for name in TABLES:
            states = [('before', original, set())]
            for selection, removed in SELECTIONS.items():
                output = args.directory / f'AutoNumber-{profile}-{name}-{selection}.accdb'
                stem = output.stem
                invoke(command + ['copy', str(original), str(output), name, selection],
                       args.directory / f'{stem}-delete.json')
                stdout = invoke(command + ['oracle', str(output)],
                                args.directory / f'{stem}-getter.json')
                model = parse(stdout)
                validate(model, name, removed)
                report['observations'].append({'profile': profile, 'table': name,
                                               'state': selection, 'artifact': pin(output),
                                               'tables': model})
                states.append((selection, output, removed))
            for state, source, removed in states:
                source_pin = pin(source)
                output = args.directory / f'AutoNumber-{profile}-{name}-{state}-append.accdb'
                invoke(command + ['copy', str(source), str(output), name, 'append'],
                       args.directory / f'{output.stem}-action.json')
                stdout = invoke(command + ['oracle', str(output)],
                                args.directory / f'{output.stem}-getter.json')
                model = parse(stdout)
                validate(model, name, removed, name)
                assert pin(source) == source_pin
                report['observations'].append({'profile': profile, 'table': name,
                                               'state': state + '-append', 'artifact': pin(output),
                                               'tables': model})
        assert pin(original) == before
    report['originals_after'] = [pin(REPOSITORY / item['path']) for item in originals]
    assert report['originals_after'] == originals
    report['observation_count'] = len(report['observations'])
    report['artifacts'].extend(pin(path) for path in sorted(args.directory.glob('*.json')))
    with args.report.open('x') as output:
        json.dump(report, output, indent=2)
        output.write('\n')
    print(f"{report['observation_count']} observations passed; {args.report}")


if __name__ == '__main__':
    main()
