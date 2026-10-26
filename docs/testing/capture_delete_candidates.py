#!/usr/bin/env python3
"""Capture inert original DELETE populations; no TableCraft or SQL oracle."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

JAR_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
PROFILES = ('V2016', 'V2019')
STATES = ('before', 'delete-one', 'delete-null', 'delete-all')
TABLES = ('Indexed', 'CodecKeys', 'FloatingKeys', 'Unindexed', 'Sentinel')
CODEC_KEYS = ('ByteKey', 'ShortKey', 'LongKey', 'BigKey', 'MoneyKey',
              'GuidKey', 'DecimalKey', 'BooleanKey')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def expected_indexes(table):
    if table == 'Indexed':
        return {'PrimaryKey', 'GroupAsc', 'GroupTwin', 'GroupDesc', 'Compound', 'UniqueScore'}
    if table == 'CodecKeys':
        result = {'PrimaryKey', 'MixedTuple'}
        keys = CODEC_KEYS
    elif table == 'FloatingKeys':
        result = set()
        keys = ('SingleKey', 'DoubleKey', 'DateKey')
    else:
        return set()
    for key in keys:
        result.add(key + 'Asc')
        result.add(key + 'Desc')
    return result


def removed_ids(state):
    if state == 'delete-all':
        return set(range(1, 7))
    if state == 'delete-one':
        return {2}
    if state == 'delete-null':
        return {3}
    return set()


def validate(item):
    path = Path(item['input'])
    state = path.stem.split('-', 2)[2]
    removed = removed_ids(state)
    rows = {}
    entries = {}
    indexes = {}
    tables = set()
    for line in item['stdout'].splitlines():
        fields = line.split('\t')
        if fields[0] == 'TABLE':
            tables.add(fields[1])
        elif fields[0] == 'ROW':
            rows.setdefault(fields[1], []).append(fields[2:])
        elif fields[0] == 'ENTRY':
            key = (fields[1], fields[2])
            entries.setdefault(key, []).append(int(fields[3]))
        elif fields[0] == 'INDEX':
            table, name = fields[1:3]
            indexes.setdefault(table, set()).add(name)
            if fields[9] != 'false' or int(fields[7]) != 1:
                raise AssertionError('Expected ordinary single-root owned index')
    if tables != set(TABLES):
        raise AssertionError('Table inventory differs')
    for table in TABLES:
        expected = []
        if table == 'Sentinel':
            expected = [91]
        else:
            limit = 3 if table == 'Unindexed' else 6
            for identifier in range(1, limit + 1):
                if identifier not in removed:
                    expected.append(identifier)
        actual = []
        for row in rows.get(table, []):
            actual.append(int(row[0]))
        if actual != expected:
            raise AssertionError((path.name, table, actual, expected))
        inventory = expected_indexes(table)
        if indexes.get(table, set()) != inventory:
            raise AssertionError('Index declaration inventory differs')
        for name in inventory:
            values = entries.get((table, name), [])
            if sorted(values) != sorted(expected):
                raise AssertionError((path.name, name, values, expected))
    for table, name in entries:
        if name not in expected_indexes(table):
            raise AssertionError('Undeclared index entries')
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    if args.report.exists():
        parser.error('Report must be fresh')
    if digest(args.jar) != JAR_SHA256:
        parser.error('Unexpected reviewed Jackcess JAR')

    helper = Path('docs/testing/DeleteCandidateFixture.java')
    files = []
    for profile in PROFILES:
        for state in STATES:
            files.append(args.inputs / f'Candidate-{profile}-{state}.accdb')
    capture_script = Path(__file__).resolve().relative_to(Path.cwd())
    pins = [helper, capture_script, args.jar, *files]
    before = {}
    for path in pins:
        before[str(path)] = digest(path)

    controls = []
    for path in files:
        command = [
            'java', '-Dstdout.encoding=UTF-8', '-cp', str(args.jar),
            str(helper), 'oracle', str(path),
        ]
        result = subprocess.run(
            command, capture_output=True, text=True, check=False, timeout=120,
        )
        controls.append({
            'input': str(path),
            'command': command,
            'status': result.returncode,
            'stdout': result.stdout,
            'stderr': result.stderr,
        })
    populations = {}
    for item in controls:
        if item['status'] == 0:
            populations[Path(item['input']).name] = validate(item)
    for profile in PROFILES:
        original = populations[f'Candidate-{profile}-before.accdb']
        for state in STATES[1:]:
            candidate = populations[f'Candidate-{profile}-{state}.accdb']
            for table, rows in candidate.items():
                originals = {}
                for row in original[table]:
                    originals[row[0]] = row
                for row in rows:
                    if row != originals[row[0]]:
                        raise AssertionError('Surviving getter payload changed')
    after = {}
    for path in pins:
        after[str(path)] = digest(path)
    generation_commands = []
    for profile in PROFILES:
        generation_commands.append([
            'java', '-Dstdout.encoding=UTF-8', '-cp', str(args.jar), str(helper),
            'generate', str(args.inputs), profile,
        ])
    failures = sum(item['status'] != 0 for item in controls)
    report = {
        'scope': 'Original public Jackcess builders/deleteRow and inert read-only getters; no native Access SQL',
        'generation_commands': generation_commands,
        'generation_note': 'Generation preceded capture. Getter-only schema/index/date-bit output was then added; no original changed. File bytes are not claimed reproducible.',
        'hashes_before': before,
        'hashes_after': after,
        'pins_unchanged': before == after,
        'controls': controls,
        'failures': failures,
    }
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({
        'controls': len(controls),
        'failures': failures,
        'pins_unchanged': report['pins_unchanged'],
        'report_sha256': digest(args.report),
    }))
    if failures or not report['pins_unchanged']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
