#!/usr/bin/env python3
"""Compare frozen CLI physical reads with the registered French stored oracle."""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess

from encoded_delete_originals import wire

ROOT = Path(__file__).resolve().parents[2]
COMPILED_REVISION = '978435fa08c6d92577b031929d8abdeb3bd4b247'
CLI_SHA256 = 'fc11c14e3bb34119888deaeef6ee5e4c6ee785679bba9ace84aaacde195c672b'
INVALID_INPUT_STATUS = 2
INDEX_HEADER_BYTES = 480
INDEX_BOUNDARY_OFFSET = 27
INDEX_PREVIOUS_OFFSET = 12
REFERENCE_WIDTH = 4
UNKNOWN_VARIANT = 1
ADDITIONAL_SOURCES = (
    'apps/tablecraft-cli/src/main.rs', 'apps/tablecraft-cli/src/output.rs',
    'apps/tablecraft-cli/src/commands/inspect/text_order.rs',
    'crates/engines/tablecraft-ace/src/error.rs',
    'crates/backends/tablecraft-access/src/source.rs',
    'crates/query/tablecraft-query/src/error.rs',
)


def pin(path):
    raw = path.read_bytes()
    return {'path': path.resolve().relative_to(ROOT).as_posix(), 'bytes': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest()}


def grid(text):
    return list(csv.reader(io.StringIO(text), delimiter='\t'))


def display(value):
    if value is None:
        return 'NULL'
    escapes = {'\\': '\\\\', '"': '\\"', '\t': '\\t', '\n': '\\n', '\r': '\\r'}
    text = ''.join(escapes.get(char, f'\\u{{{ord(char):04x}}}'
                              if ord(char) < 32 or 127 <= ord(char) <= 159 else char)
                   for char in value)
    return '"' + text + '"'


def inspector(table, model, mode):
    if mode == 'schema':
        sort = model['sort']
        family = 'Unqualified' if table == 'French' else 'General legacy'
        return [['Table', table], ['Declared rows', '38'],
                ['Scalar columns', 'Supported carriers; rows not inspected'], ['Unsupported columns', '0'],
                ['Column', 'Native type', 'Storage bytes', 'Flags', 'Extended flags', 'Scalar carrier'],
                ['ID', '4', '4', '0x03', '0x00', 'Int32'],
                ['Word', '10', '510', '0x02', '0x00', 'Text'],
                ['Text column', 'Sort LCID', 'Sort version', 'Sort variant', 'Sort family'],
                ['Word', str(sort['lcid']), '0', '0x00', family]]
    if mode == 'check':
        return [['Table', table], ['Rows checked', '38'], ['Indexes checked', '3'],
                ['Indexes not checked', '0'], ['Index', 'Status', 'Index constraints'],
                *[[name, 'Checked', 'Checked: exact unique/required'] for name in ('Asc', 'Twin', 'Desc')]]
    result = [['Table', table],
              ['Index', 'Logical', 'Physical', 'Root page', 'Type', 'Flags', 'Primary', 'Unique',
               'Required key', 'Ignore NULLs', 'Column', 'Descending']]
    for logical, name in enumerate(('Asc', 'Twin', 'Desc')):
        index = model['indexes'][name]
        result.append([name, str(logical), str(index['physical']), str(index['root']), '0', '0x80',
                       'FALSE', 'FALSE', 'FALSE', 'FALSE', 'Word', 'TRUE' if name == 'Desc' else 'FALSE'])
    return result


def query_cases(table, model):
    rows = {row['id']: row['value'] for row in model['rows']}
    yield ('sequential-values', f'SELECT ID,Word FROM [{table}]', None,
           [['ID', 'Word'], *[[str(key), display(value)] for key, value in rows.items()]])
    for index in ('Asc', 'Twin', 'Desc'):
        order = model['indexes'][index]['order']
        yield (index + '-ids', f'SELECT ID FROM [{table}]', index,
               [['ID'], *[[str(key)] for key in order]])
        yield (index + '-null-filter', f'SELECT ID FROM [{table}] WHERE Word IS NULL', index,
               [['ID'], *[[str(key)] for key in order if rows[key] is None]])
        yield (index + '-values', f'SELECT ID,Word FROM [{table}]', index,
               [['ID', 'Word'], *[[str(key), display(rows[key])] for key in order]])


def damaged_copies(source, model, directory):
    raw = source.read_bytes()
    root = model['indexes']['Asc']['root']
    base = root * wire.PAGE_SIZE
    page = wire.page(raw, root)
    assert page[0] == 4 and wire.u16(page, wire.INDEX_PREFIX_OFFSET) == 0
    ends = [number * 8 + bit for number, mask in enumerate(page[INDEX_BOUNDARY_OFFSET:INDEX_HEADER_BYTES])
            for bit in range(8) if mask & (1 << bit)]
    assert len(ends) == 38
    # First NULL entry, then empty Text. Both are independently recorded originals.
    assert page[INDEX_HEADER_BYTES] == 0
    empty = INDEX_HEADER_BYTES + ends[0]
    assert page[empty:empty + 3] == bytes([127, 1, 0])
    # Change the complete empty key's terminator, or substitute its row reference.
    mutations = [('wrong-key', base + empty + 1, bytes([2]), 'corrupt'),
                 ('wrong-row', base + INDEX_HEADER_BYTES + ends[1] - REFERENCE_WIDTH,
                  page[INDEX_HEADER_BYTES + ends[0] - REFERENCE_WIDTH:INDEX_HEADER_BYTES + ends[0]], 'corrupt'),
                 ('unknown-sort', model['sort']['offset'] + 2, bytes([UNKNOWN_VARIANT]), 'unsupported')]
    for name, offset, replacement, error in mutations:
        changed = bytearray(raw)
        before = bytes(changed[offset:offset + len(replacement)])
        assert before != replacement
        changed[offset:offset + len(replacement)] = replacement
        path = directory / (source.stem + '-' + name + '.accdb')
        with path.open('xb') as output:
            output.write(changed)
        yield name, path, error, {'offset': offset, 'before_hex': before.hex(), 'after_hex': replacement.hex()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--cli-sha256', required=True)
    parser.add_argument('--build-receipt', type=Path, required=True)
    parser.add_argument('--oracle', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    assert pin(args.cli)['sha256'] == args.cli_sha256 == CLI_SHA256
    assert not args.report.exists()
    args.output.mkdir(parents=True, exist_ok=False)
    receipt = json.loads(args.build_receipt.read_text())
    assert receipt['compiled_revision'] == COMPILED_REVISION
    assert receipt['frozen_cli']['sha256'] == CLI_SHA256
    compiled = {}
    for item in receipt['source_pins']:
        raw = subprocess.check_output(['git', 'show', f"{COMPILED_REVISION}:{item['path']}"], timeout=30)
        assert len(raw) == item['bytes'] and hashlib.sha256(raw).hexdigest() == item['sha256']
        compiled[item['path']] = item
    for path in ADDITIONAL_SOURCES:
        raw = subprocess.check_output(['git', 'show', f'{COMPILED_REVISION}:{path}'], timeout=30)
        compiled[path] = {'path': path, 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}
    raw_main = subprocess.check_output(['git', 'show', f'{COMPILED_REVISION}:apps/tablecraft-cli/src/main.rs'], timeout=30)
    assert f'const INVALID_INPUT_STATUS: u8 = {INVALID_INPUT_STATUS};'.encode() in raw_main
    oracle = json.loads(args.oracle.read_text())
    for item in oracle['artifacts']:
        assert pin(ROOT / item['path']) == item
    artifacts = [args.cli, args.build_receipt, args.oracle, Path(__file__), Path(wire.__file__)]
    artifacts.extend(ROOT / item['original']['path'] for item in oracle['profiles'])
    before = [pin(path) for path in artifacts]
    observations = []
    query_rows = query_cells = 0

    def execute(profile, name, command, expected=None, error=None, mutation=None):
        nonlocal query_rows, query_cells
        result = subprocess.run(command, capture_output=True, text=True, timeout=60)
        disagreement = None
        try:
            if error:
                assert result.returncode == INVALID_INPUT_STATUS and not result.stdout
                classes = {'corrupt': ('query source failed',),
                           'unsupported': ('not implemented', 'not supported')}
                assert any(value in result.stderr.lower() for value in classes[error])
            else:
                assert result.returncode == 0 and not result.stderr
                assert grid(result.stdout) == expected
                if 'query' in command:
                    query_rows += len(expected) - 1
                    query_cells += sum(map(len, expected[1:]))
        except AssertionError:
            disagreement = 'Exit/status/classification or exact table result differs'
        record = {'profile': profile, 'case': name, 'command': command,
                  'exit_code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr,
                  'expected': expected, 'expected_error_class': error, 'mutation': mutation,
                  'agrees': disagreement is None, 'disagreement': disagreement}
        observations.append(record)
        with (args.output / f'{profile}-{name}.json').open('x') as output:
            json.dump(record, output, indent=2, ensure_ascii=True)
            output.write('\n')

    for entry in oracle['profiles']:
        profile = entry['profile']
        source = ROOT / entry['original']['path']
        assert pin(source) == entry['original']
        for table, model in entry['tables'].items():
            for mode, flags in [('schema', []), ('indexes', ['--indexes']), ('check', ['--check'])]:
                command = [str(args.cli), '--format', 'tsv', 'inspect', str(source), '--table', table, *flags]
                execute(profile, table + '-' + mode, command, inspector(table, model, mode))
            for name, sql, index, expected in query_cases(table, model):
                command = [str(args.cli), '--format', 'tsv', 'query', 'run', sql,
                           '--database', str(source), '--collation', 'ordinal']
                if index:
                    command.extend(['--index', table, index])
                execute(profile, table + '-' + name, command, expected)
        for name, damaged, error, mutation in damaged_copies(source, entry['tables']['French'], args.output):
            command = [str(args.cli), '--format', 'tsv', 'query', 'run', 'SELECT ID FROM French',
                       '--database', str(damaged), '--collation', 'ordinal', '--index', 'French', 'Asc']
            execute(profile, name, command, error=error, mutation=mutation)
    assert [pin(path) for path in artifacts] == before
    assert len(observations) == 58
    report = {'schema_version': 1, 'compiled_revision': COMPILED_REVISION,
              'scope': 'Frozen CLI physical French/General index reads under explicit ordinal query policy, not French SQL comparison or range pushdown',
              'controls': len(observations), 'successes': sum(item['agrees'] and not item['expected_error_class'] for item in observations),
              'refusals': sum(item['agrees'] and item['expected_error_class'] is not None for item in observations),
              'disagreements': sum(not item['agrees'] for item in observations),
              'query_rows': query_rows, 'query_cells': query_cells,
              'immutable_artifacts_before_after': before,
              'selected_compiled_source_objects': compiled,
              'captures': [pin(path) for path in sorted(args.output.glob('*.json'))],
              'damaged_copies': [pin(path) for path in sorted(args.output.glob('*.accdb'))]}
    with args.report.open('x') as output:
        json.dump(report, output, indent=2)
        output.write('\n')
    print(json.dumps({key: report[key] for key in ('controls', 'successes', 'refusals', 'disagreements', 'query_rows', 'query_cells')}))
    if report['disagreements']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
