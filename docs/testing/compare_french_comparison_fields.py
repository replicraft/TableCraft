#!/usr/bin/env python3
"""Compare frozen CLI field policies with bounded independent engineering models."""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

from capture_french_comparison import parse
from compare_french_physical_reads import display
from encoded_delete_originals import wire
from french_comparison_latin_model import load
from french_field_comparison.model import FieldModel, cases, short_hex

ROOT = Path(__file__).resolve().parents[2]
REVISION = '596d40ea41a4bafefbd5b34af4ef35a8a4e2cde4'
CLI_SHA256 = 'bcff00ad245c7a70582db222fa48e5eb2f796bd944d5ccbeefe759fbd010a576'
INVALID_INPUT_STATUS = 2
COMPARISON_BYTES = 64 * 1024 * 1024
COMPARISON_STEPS = 10_000_000
NATIVE_BYTES = 64 * 1024 * 1024
NATIVE_STEPS = 10_000_000


def pin(path):
    raw = path.read_bytes()
    return {'path': path.resolve().relative_to(ROOT).as_posix(), 'bytes': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest()}


def scalar(value):
    if value is None:
        return 'NULL'
    if isinstance(value, bool):
        return 'TRUE' if value else 'FALSE'
    if isinstance(value, str):
        return display(value)
    return str(value)


def short_keys(path):
    result = {}
    for line in path.read_text().splitlines():
        lcid, value, key = line.split('\t')
        identity = (int(lcid), value)
        assert identity not in result
        result[identity] = bytes.fromhex(key)
    return result


def physical_short_proof(raw, item, keys):
    locations = {tuple(location): identifier for identifier, location in item['rows'].items()}
    index = item['indexes']['FrenchShortAsc']
    entries, leaves, nodes = wire.decode_tree(raw, index['root'], item['definition_page'])
    assert len(entries) == 38 and len(leaves) == 1 and not nodes
    assert index['order'] == [locations[(page, slot)] for _, page, slot in entries]
    for key, page, slot in entries:
        identifier = locations[(page, slot)]
        value = item['values'][identifier]['FrenchShort']
        expected = bytes([0]) if value is None else bytes([127]) + keys[(1036, short_hex(value.decode('utf-16-le')))]
        assert bytes.fromhex(key) == expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('cli', 'build-receipt', 'fields', 'short-keys', 'latin-oracle', 'defaults', 'output', 'report'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--cli-sha256', required=True)
    args = parser.parse_args()
    assert not args.report.exists()
    assert pin(args.cli)['sha256'] == args.cli_sha256 == CLI_SHA256
    args.output.mkdir(parents=True, exist_ok=False)
    receipt = json.loads(args.build_receipt.read_text())
    assert receipt['compiled_revision'] == REVISION
    assert receipt['frozen_cli']['sha256'] == CLI_SHA256
    compiled = []
    for item in receipt['source_pins']:
        raw = subprocess.check_output(['git', 'show', f"{REVISION}:{item['path']}"], timeout=30)
        assert len(raw) == item['bytes'] and hashlib.sha256(raw).hexdigest() == item['sha256']
        compiled.append(item)
    fields = json.loads(args.fields.read_text())
    assert fields['counts'] == {'originals': 2, 'rows': 76, 'text_memo_cells': 304, 'input_storage_differences': 0}
    defaults = json.loads(args.defaults.read_text())
    assert {entry['profile']: entry['stdout'].strip() for entry in defaults} == {
        'V2016': '1033:0:1\t1036:0:0', 'V2019': '1033:0:0\t1036:0:0'}
    keys = short_keys(args.short_keys)
    weights, _ = load(args.latin_oracle)
    artifacts = [args.cli, args.build_receipt, args.fields, args.short_keys, args.latin_oracle,
                 args.defaults, Path(__file__), ROOT / 'docs/testing/capture_french_comparison.py',
                 ROOT / 'docs/testing/compare_french_physical_reads.py',
                 ROOT / 'docs/testing/french_field_comparison/model.py',
                 ROOT / 'docs/testing/french_comparison_latin_model.py', Path(wire.__file__)]
    for entry in fields['profiles']:
        for item in (entry['original'], entry['getter']):
            path = ROOT / item['path']
            assert pin(path) == item
            artifacts.append(path)
    before = [pin(path) for path in artifacts]
    controls = []
    rows_compared = cells_compared = 0
    expected_models = []
    for entry in fields['profiles']:
        original = ROOT / entry['original']['path']
        captured = parse(json.loads((ROOT / entry['getter']['path']).read_text())['stdout'])
        physical_short_proof(original.read_bytes(), captured, keys)
        model = FieldModel(captured, keys, weights)
        queries = list(cases(model))
        assert len(queries) == len({item[0] for item in queries}) == 28
        expected_models.append({'profile': entry['profile'], 'cases': [item[0] for item in queries]})
        for declared in (False, True):
            for name, sql, headings, expected, refusal, extra in queries:
                options = {'--collation': 'database', '--comparison-bytes': str(COMPARISON_BYTES),
                           '--comparison-steps': str(COMPARISON_STEPS)}
                assert len(extra) % 2 == 0
                options.update(zip(extra[::2], extra[1::2]))
                command = [str(args.cli), '--format', 'tsv', 'query', 'run', sql,
                           '--database', str(original), '--native-bytes', str(NATIVE_BYTES),
                           '--native-steps', str(NATIVE_STEPS)]
                for flag, value in options.items():
                    command.extend([flag, value])
                if declared:
                    command.append('--declared-arithmetic')
                result = subprocess.run(command, capture_output=True, text=True, timeout=120)
                disagreement = None
                try:
                    if refusal:
                        assert result.returncode == INVALID_INPUT_STATUS and not result.stdout
                        assert refusal.lower() in result.stderr.lower()
                    else:
                        assert result.returncode == 0 and not result.stderr
                        actual = list(csv.reader(io.StringIO(result.stdout), delimiter='\t'))
                        wanted = [headings, *[[scalar(value) for value in row] for row in expected]]
                        assert actual == wanted
                        rows_compared += len(expected)
                        cells_compared += sum(map(len, expected))
                except AssertionError:
                    disagreement = 'Exit/refusal class or exact modeled result differs'
                record = {'profile': entry['profile'], 'declared': declared, 'case': name,
                          'command': command, 'exit_code': result.returncode,
                          'stdout': result.stdout, 'stderr': result.stderr,
                          'expected_headings': headings, 'expected_rows': expected,
                          'expected_refusal': refusal, 'agrees': disagreement is None,
                          'disagreement': disagreement}
                path = args.output / f"{entry['profile']}-{'declared' if declared else 'ordinary'}-{name}.json"
                with path.open('x') as stream:
                    json.dump(record, stream, indent=2, ensure_ascii=True)
                    stream.write('\n')
                controls.append(record)
    assert len(controls) == 112
    assert [pin(path) for path in artifacts] == before
    report = {'scope': 'Actual native field policy composition with independent short keys and bounded Latin long model; not native SQL/NLS qualification',
              'compiled_revision': REVISION, 'controls': len(controls),
              'successes': sum(item['agrees'] and item['expected_refusal'] is None for item in controls),
              'refusals': sum(item['agrees'] and item['expected_refusal'] is not None for item in controls),
              'disagreements': sum(not item['agrees'] for item in controls),
              'rows_compared': rows_compared, 'cells_compared': cells_compared,
              'comparison_positive_bytes': COMPARISON_BYTES, 'comparison_positive_steps': COMPARISON_STEPS,
              'explicit_native_bytes': NATIVE_BYTES, 'explicit_native_steps': NATIVE_STEPS,
              'immutable_artifacts_before_after': before, 'compiled_source_objects': compiled,
              'expected_case_inventory': expected_models,
              'captures': [pin(path) for path in sorted(args.output.glob('*.json'))]}
    with args.report.open('x') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    print(json.dumps({key: report[key] for key in ('controls', 'successes', 'refusals', 'disagreements', 'rows_compared', 'cells_compared')}))
    if report['disagreements']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
