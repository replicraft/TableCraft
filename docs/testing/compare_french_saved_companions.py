#!/usr/bin/env python3
"""Compare inert saved reconstruction and explicit named/derived field execution."""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

from capture_french_comparison import parse as field_model
from capture_french_saved_companions import EXPECTED_SQL
from compare_french_comparison_fields import COMPARISON_BYTES, COMPARISON_STEPS, NATIVE_BYTES, NATIVE_STEPS, REVISION, CLI_SHA256, scalar, short_keys
from french_comparison_latin_model import load
from french_field_comparison.model import FieldModel

ROOT = Path(__file__).resolve().parents[2]
SAVED = 'FrenchFields'
INVALID_INPUT_STATUS = 2
ADDITIONAL_SOURCES = ('apps/tablecraft-cli/src/commands/query/saved.rs',)


def pin(path):
    data = path.read_bytes()
    return {'path': path.resolve().relative_to(ROOT).as_posix(), 'bytes': len(data),
            'sha256': hashlib.sha256(data).hexdigest()}


def execution_cases(model):
    rows = model.rows
    yield ('direct-saved-values', None, ['ID', 'FrenchShort', 'FrenchMemo'],
           [[identifier, row['FrenchShort'], row['FrenchMemo']] for identifier, row in rows.items()], None)
    short_order = [[identifier] for identifier in model.ordered_ids('FrenchShort')]
    long_order = [[identifier] for identifier in model.ordered_ids('FrenchMemo', list(range(33, 39))) ]
    yield ('named-short-sort', f'SELECT ID FROM {SAVED} ORDER BY FrenchShort,ID', ['ID'], short_order, None)
    yield ('named-long-memo-sort', f'SELECT ID FROM {SAVED} WHERE ID>=33 AND ID<=38 ORDER BY FrenchMemo,ID', ['ID'], long_order, None)
    yield ('derived-named-short', f'SELECT Q.ID FROM (SELECT ID,FrenchShort AS W FROM {SAVED}) AS Q ORDER BY Q.W,Q.ID', ['ID'], short_order, None)
    yield ('derived-named-memo', f'SELECT Q.ID FROM (SELECT ID,FrenchMemo AS M FROM {SAVED} WHERE ID>=33 AND ID<=38) AS Q ORDER BY Q.M,Q.ID', ['ID'], long_order, None)
    groups = {}
    for row in rows.values():
        key = model.short_key(row['FrenchShort'])
        if key not in groups:
            groups[key] = [row['FrenchShort'], 0]
        groups[key][1] += 1
    yield ('named-short-groups', f'SELECT FrenchShort AS V,Count(*) AS N FROM {SAVED} GROUP BY FrenchShort', ['V', 'N'], list(groups.values()), None)
    yield ('named-like-refusal', f"SELECT ID FROM {SAVED} WHERE FrenchShort LIKE 'co*'", [], [], 'expression form is not implemented')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('cli', 'build-receipt', 'metadata', 'fields', 'short-keys', 'latin-oracle', 'output', 'report'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--cli-sha256', required=True)
    args = parser.parse_args()
    assert pin(args.cli)['sha256'] == args.cli_sha256 == CLI_SHA256
    assert not args.report.exists()
    args.output.mkdir(parents=True, exist_ok=False)
    build = json.loads(args.build_receipt.read_text())
    assert build['compiled_revision'] == REVISION and build['frozen_cli']['sha256'] == CLI_SHA256
    compiled = {}
    for item in build['source_pins']:
        data = subprocess.check_output(['git', 'show', f"{REVISION}:{item['path']}"], timeout=30)
        assert len(data) == item['bytes'] and hashlib.sha256(data).hexdigest() == item['sha256']
        compiled[item['path']] = item
    for path in ADDITIONAL_SOURCES:
        data = subprocess.check_output(['git', 'show', f'{REVISION}:{path}'], timeout=30)
        compiled[path] = {'path': path, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}
    metadata = json.loads(args.metadata.read_text())
    fields = json.loads(args.fields.read_text())
    keys = short_keys(args.short_keys)
    weights, _ = load(args.latin_oracle)
    artifacts = [args.cli, args.build_receipt, args.metadata, args.fields, args.short_keys,
                 args.latin_oracle, Path(__file__),
                 ROOT / 'docs/testing/capture_french_comparison.py',
                 ROOT / 'docs/testing/capture_french_saved_companions.py',
                 ROOT / 'docs/testing/compare_french_comparison_fields.py',
                 ROOT / 'docs/testing/french_field_comparison/model.py',
                 ROOT / 'docs/testing/french_comparison_latin_model.py']
    for entry in metadata['profiles']:
        for item in (entry['source'], entry['companion']):
            path = ROOT / item['path']
            assert pin(path) == item
            artifacts.append(path)
    for entry in fields['profiles']:
        path = ROOT / entry['getter']['path']
        assert pin(path) == entry['getter']
        artifacts.append(path)
    before = [pin(path) for path in artifacts]
    controls = []
    rows_compared = cells_compared = 0

    def record(profile, name, command, expected, error=None, declared=None, syntax=False):
        nonlocal rows_compared, cells_compared
        result = subprocess.run(command, capture_output=True, text=True, timeout=120)
        disagreement = None
        try:
            if error:
                assert result.returncode == INVALID_INPUT_STATUS and not result.stdout
                assert error in result.stderr
            elif syntax:
                assert result.returncode == 0 and not result.stderr
                if expected is not None:
                    assert result.stdout == expected
            else:
                assert result.returncode == 0 and not result.stderr
                assert list(csv.reader(io.StringIO(result.stdout), delimiter='\t')) == expected
                rows_compared += len(expected) - 1
                cells_compared += sum(map(len, expected[1:]))
        except AssertionError:
            disagreement = 'Exit, reconstruction, exact values/order or refusal differs'
        item = {'profile': profile, 'case': name, 'declared': declared, 'command': command,
                'exit_code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr,
                'expected': expected, 'expected_refusal': error, 'syntax_only': syntax,
                'agrees': disagreement is None, 'disagreement': disagreement}
        with (args.output / f"{profile}-{name}-{'declared' if declared else 'ordinary'}.json").open('x') as stream:
            json.dump(item, stream, indent=2, ensure_ascii=True)
            stream.write('\n')
        controls.append(item)
        return result.stdout

    for companion in metadata['profiles']:
        profile = companion['profile']
        source = ROOT / companion['companion']['path']
        saved = companion['saved']['queries'][0]
        assert saved['name'] == SAVED and saved['sql'] == EXPECTED_SQL
        canonical = record(profile, 'independent-sql-parse', [str(args.cli), 'query', 'parse', saved['sql']], None, syntax=True)
        record(profile, 'native-saved-reconstruct', [str(args.cli), 'query', 'saved', '--database', str(source), SAVED], canonical, syntax=True)
        original = next(item for item in fields['profiles'] if item['profile'] == profile)
        captured = field_model(json.loads((ROOT / original['getter']['path']).read_text())['stdout'])
        queries = list(execution_cases(FieldModel(captured, keys, weights)))
        assert len(queries) == 7
        for declared in (False, True):
            for name, sql, headings, values, refusal in queries:
                command = [str(args.cli), '--format', 'tsv', 'query', 'run']
                if sql is None:
                    command.extend(['--saved', SAVED])
                else:
                    command.append(sql)
                command.extend(['--database', str(source), '--collation', 'database',
                                '--native-bytes', str(NATIVE_BYTES), '--native-steps', str(NATIVE_STEPS),
                                '--comparison-bytes', str(COMPARISON_BYTES), '--comparison-steps', str(COMPARISON_STEPS)])
                if refusal:
                    command.extend(['--like-mode', 'ansi89'])
                if declared:
                    command.append('--declared-arithmetic')
                expected = [headings, *[[scalar(value) for value in row] for row in values]]
                record(profile, name, command, expected, refusal, declared)
    assert len(controls) == 32
    assert [pin(path) for path in artifacts] == before
    report = {'scope': 'Independent saved SELECT reconstruction plus explicit native saved/named/derived French field policy execution; engineering only',
              'compiled_revision': REVISION, 'controls': len(controls),
              'syntax_controls': sum(item['syntax_only'] for item in controls),
              'successful_execution_controls': sum(item['agrees'] and not item['syntax_only'] and item['expected_refusal'] is None for item in controls),
              'refusal_controls': sum(item['agrees'] and item['expected_refusal'] is not None for item in controls),
              'disagreements': sum(not item['agrees'] for item in controls),
              'rows_compared': rows_compared, 'cells_compared': cells_compared,
              'immutable_artifacts_before_after': before, 'compiled_source_objects': compiled,
              'captures': [pin(path) for path in sorted(args.output.glob('*.json'))]}
    with args.report.open('x') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    print(json.dumps({key: report[key] for key in ('controls', 'syntax_controls', 'successful_execution_controls', 'refusal_controls', 'disagreements', 'rows_compared', 'cells_compared')}))
    if report['disagreements']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
