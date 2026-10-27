"""Frozen RESTRICT candidate replay with separate final-image and public-action metrics."""

import argparse
import json
from pathlib import Path
import subprocess
import sys
import traceback

from getters import parse_getters
from reference import tables_list, violations
from support import (JAR_SHA256, REPOSITORY_ROOT, canonical, fresh_report, oracle_command, pin, verify, wire)

sys.path.insert(0, str(REPOSITORY_ROOT / 'docs/testing/ignored_delete_originals'))
from atomic import check as atomic_check

EXPECTED_CONTROLS = 84
PROCESS_TIMEOUT_SECONDS = 120


def plain(value):
    return json.loads(json.dumps(value))


def run(argv, directory, label):
    stdout, stderr = directory / (label + '.stdout'), directory / (label + '.stderr')
    with stdout.open('xb') as out, stderr.open('xb') as err:
        completed = subprocess.run(argv, stdout=out, stderr=err, timeout=PROCESS_TIMEOUT_SECONDS)
    return {'argv': argv, 'exit_code': completed.returncode,
            'stdout': pin(stdout), 'stderr': pin(stderr)}


def semantic_check(before, after, table, selected):
    assert canonical(before['relationships']) == canonical(after['relationships'])
    assert before['native_types'] == after['native_types']
    assert not violations(after)
    for name, old in before['tables'].items():
        current = after['tables'][name]
        selected_here = selected if name == table else set()
        expected_rows = {identifier: row for identifier, row in old['rows'].items()
                         if identifier not in selected_here}
        assert canonical(current['rows']) == canonical(expected_rows)
        assert current['count'] == old['count'] - len(selected_here)
        for field in ['definition', 'columns', 'column_count', 'max_columns',
                      'properties', 'children', 'catalog', 'active', 'relationships']:
            assert canonical(current[field]) == canonical(old[field])
        assert set(current['indexes']) == set(old['indexes'])
        for index_name, old_index in old['indexes'].items():
            new_index = current['indexes'][index_name]
            for field in ['physical', 'root', 'usage', 'columns', 'historical', 'counter_offset',
                          'unique', 'ignore_nulls', 'required']:
                assert canonical(new_index[field]) == canonical(old_index[field])
            expected_order = [identifier for identifier in old_index['order'] if identifier not in selected_here]
            refs = {(row['page'], row['slot']) for row in expected_rows.values()}
            expected_keys = [key for key in old_index['keys'] if (key[1], key[2]) in refs]
            assert new_index['order'] == expected_order
            assert new_index['keys'] == expected_keys
            assert new_index['eligible_ids'] == sorted(expected_order)
            assert new_index['entries'] == len(expected_order)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--references', type=Path, required=True)
    parser.add_argument('--baseline-dir', type=Path, required=True)
    parser.add_argument('--helper-source', type=Path, required=True)
    parser.add_argument('--helper-dir', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--example', type=Path, required=True)
    parser.add_argument('--example-sha256', required=True)
    parser.add_argument('--compiled-revision', required=True)
    parser.add_argument('--workdir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    assert Path.cwd().resolve() == REPOSITORY_ROOT
    for name in ['references', 'baseline_dir', 'helper_source', 'helper_dir', 'jar', 'example', 'workdir', 'report']:
        path = getattr(args, name)
        setattr(args, name, Path(path.resolve().relative_to(REPOSITORY_ROOT).as_posix()))
    assert pin(args.jar)['sha256'] == JAR_SHA256
    assert pin(args.example)['sha256'] == args.example_sha256
    assert len(args.compiled_revision) == 40 and all(c in '0123456789abcdef' for c in args.compiled_revision)
    assert not args.report.exists() and not args.workdir.exists()
    reference_report = json.loads(args.references.read_text())
    assert reference_report['controls'] == EXPECTED_CONTROLS
    assert reference_report['disagreements'] == 6
    reference_cases = reference_report['results']
    source_pins = {item['input']['path']: item['input'] for item in reference_report['originals']}
    source_snapshots, reference_snapshots = {}, {}
    for item in reference_report['originals']:
        path = item['input']['path']
        verify(item['input'])
        snapshot = parse_getters(args.baseline_dir / (item['profile'] + '-before-getters.tsv'), Path(path).read_bytes())
        assert not violations(snapshot)
        source_snapshots[path] = snapshot
        reference_snapshots[path] = snapshot
    for case in reference_cases:
        for field in ['source', 'reference', 'request', 'getter']:
            verify(case[field])
        source_pins[case['reference']['path']] = case['reference']
    args.workdir.mkdir(parents=True, exist_ok=False)
    commands, results, accepted = [], [], {}

    for index, case in enumerate(reference_cases):
        label = f"{index:02}-{case['profile']}-{case['table']}-{case['action']}"
        reference_source = case['source']['path']
        prior = case['candidate_source_case']
        if prior is None:
            source = Path(reference_source)
            before_snapshot = source_snapshots[reference_source]
        else:
            assert prior in accepted, ('previous successful candidate unavailable', prior)
            source, before_snapshot = accepted[prior]
        before = source.read_bytes()
        source_pin = pin(source)
        if reference_source not in reference_snapshots:
            prior_reference = next(item for item in reference_cases
                                   if item['reference']['path'] == reference_source)
            reference_snapshots[reference_source] = parse_getters(
                Path(prior_reference['getter']['path']), Path(reference_source).read_bytes())
        expected_source = reference_snapshots[reference_source]
        # Histories use previous successful candidates, not sequential-writer image bytes.
        for name, table in before_snapshot['tables'].items():
            expected = expected_source['tables'][name]
            for field in ['rows', 'count', 'columns']:
                assert canonical(table[field]) == canonical(expected[field])
            for name_index, old_index in table['indexes'].items():
                for field in ['keys', 'order', 'historical']:
                    assert canonical(old_index[field]) == canonical(expected['indexes'][name_index][field])
        selected = set(case['selected_ids'])
        predicted = not violations(before_snapshot, case['table'], selected)
        assert predicted == case['expected_success']
        table = before_snapshot['tables'][case['table']]
        ordered = sorted(table['rows'].items(), key=lambda item: (item[1]['page'], item[1]['slot']))
        request = args.workdir / (label + '.tsv')
        with request.open('x') as output:
            for ordinal, (identifier, row) in enumerate(ordered):
                if identifier in selected:
                    payload = wire.row(before, (row['page'] << wire.REFERENCE_SLOT_BITS) | row['slot'])
                    output.write(f'{ordinal}\t{payload.hex()}\n')
        target = args.workdir / (label + '.accdb')
        command = run([str(args.example), str(source), str(target), case['table'], '--delete-file', str(request)],
                      args.workdir, label)
        command_index = len(commands)
        commands.append(command)
        success = command['exit_code'] == 0 and target.exists()
        stdout = Path(command['stdout']['path']).read_bytes()
        stderr = Path(command['stderr']['path']).read_text()
        missing_parent = (command['exit_code'] != 0 and not target.exists()
                          and stdout == b'' and 'MissingParent' in stderr)
        result = {'profile': case['profile'], 'table': case['table'], 'selected_ids': case['selected_ids'],
                  'action': case['action'], 'history': case['history'], 'candidate_source_case': prior,
                  'source': source_pin, 'command': command_index, 'request': pin(request),
                  'expected_final_model_success': predicted, 'actual_candidate_success': success,
                  'actual_missing_parent_refusal': missing_parent,
                  'public_reference_success': case['actual_success'],
                  'public_reference_agreed_with_model': case['agreed'],
                  'final_model_agreed': False, 'public_action_agreed': False}
        try:
            if predicted:
                assert success and stdout.decode().strip() == f'candidate_deleted_rows={len(selected)}', stderr
                getter = run(oracle_command(args, target, tables_list()), args.workdir, label + '-getter')
                result['getter_command'] = len(commands)
                commands.append(getter)
                assert getter['exit_code'] == 0
                after = target.read_bytes()
                actual = parse_getters(Path(getter['stdout']['path']), after)
                semantic_check(before_snapshot, actual, case['table'], selected)
                preservation = atomic_check(before, after, plain(table), actual['tables'][case['table']], selected)
                result.update({'candidate': pin(target), 'getter': getter['stdout'],
                               'atomic_preservation': preservation,
                               'independent_final_membership_valid': True,
                               'exact_selected_target_roots_removed': True})
                if case['actual_success']:
                    public_after = parse_getters(Path(case['getter']['path']), Path(case['reference']['path']).read_bytes())
                    for name, actual_table in actual['tables'].items():
                        assert canonical(actual_table['rows']) == canonical(public_after['tables'][name]['rows'])
                        for name_index, actual_index in actual_table['indexes'].items():
                            for field in ['keys', 'order', 'eligible_ids']:
                                assert canonical(actual_index[field]) == canonical(public_after['tables'][name]['indexes'][name_index][field])
                accepted[index] = (target, actual)
            else:
                assert missing_parent, stderr
            result['final_model_agreed'] = True
            result['public_action_agreed'] = success == case['actual_success']
        except (AssertionError, ValueError, KeyError) as error:
            result['failure'] = str(error)
            result['traceback'] = traceback.format_exc()
        results.append(result)
        assert pin(source) == source_pin
        for source_record in source_pins.values():
            verify(source_record)
        print(index, case['profile'], case['table'], case['action'],
              'final_model', result['final_model_agreed'], 'public_action', result['public_action_agreed'],
              'exit', command['exit_code'], flush=True)
    report = {'scope': 'Explicit final-image engineering DELETE versus separately reported public per-row API behavior; no native SQL timing or Access save qualification',
              'example': pin(args.example), 'compiled_revision': args.compiled_revision,
              'reference_report': pin(args.references), 'helper': pin(args.helper_source), 'jar': pin(args.jar),
              'commands': commands, 'results': results, 'controls': len(results),
              'final_model_disagreements': sum(not item['final_model_agreed'] for item in results),
              'public_action_disagreements': sum(not item['public_action_agreed'] for item in results),
              'originals_references_unchanged': True,
              'shared_atomic_checker': pin(Path(atomic_check.__code__.co_filename)),
              'shared_wire_reader': pin(Path(wire.__file__)),
              'tool_sources': [pin(path) for path in sorted(Path(__file__).parent.glob('*.py'))]}
    fresh_report(args.report, report)
    print('controls', len(results), 'final_model_disagreements', report['final_model_disagreements'],
          'public_action_disagreements', report['public_action_disagreements'], flush=True)
    if report['final_model_disagreements']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
