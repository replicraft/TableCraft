"""Replay all fifty original selectors against one explicitly frozen DELETE example."""

import argparse
import json
from pathlib import Path
import subprocess
import traceback

from atomic import check as atomic_check
from getters import parse_getters
from support import (GETTER_CLASS_FILE, JAR_SHA256, PROCESS_TIMEOUT_SECONDS,
                     REPOSITORY_ROOT, canonical, fresh_report, oracle_command, pin, verify, wire)

EXPECTED_CONTROLS = 50


def relative_path(path):
    return Path(path.resolve().relative_to(REPOSITORY_ROOT).as_posix())


def run(argv, stdout, stderr):
    with stdout.open('xb') as out, stderr.open('xb') as err:
        completed = subprocess.run(argv, stdout=out, stderr=err, timeout=PROCESS_TIMEOUT_SECONDS)
    return {'argv': argv, 'exit_code': completed.returncode,
            'stdout': pin(stdout), 'stderr': pin(stderr)}


def semantic_check(original, actual, reference, selected):
    old_rows = original['rows']
    expected_rows = {identifier: row for identifier, row in old_rows.items()
                     if int(identifier) not in selected}
    assert canonical(actual['rows']) == canonical(expected_rows)
    assert canonical(actual['rows']) == canonical(reference['rows'])
    assert actual['count'] == original['count'] - len(selected) == reference['count']
    for field in ['definition', 'columns', 'column_count', 'max_columns',
                  'properties', 'children', 'active', 'catalog', 'relationships']:
        assert canonical(actual[field]) == canonical(original[field])
    assert set(actual['indexes']) == set(original['indexes']) == set(reference['indexes'])
    for name, old in original['indexes'].items():
        current = actual['indexes'][name]
        public_reference = reference['indexes'][name]
        for field in ['physical', 'root', 'historical', 'counter_offset',
                      'columns', 'unique', 'ignore_nulls', 'required', 'usage']:
            assert canonical(current[field]) == canonical(old[field])
        for field in ['keys', 'order', 'eligible_ids', 'entries']:
            assert canonical(current[field]) == canonical(public_reference[field])
        expected_order = [identifier for identifier in old['order'] if identifier not in selected]
        assert current['order'] == expected_order


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference-report', type=Path, required=True)
    parser.add_argument('--transition-audit', type=Path, required=True)
    parser.add_argument('--helper-dir', type=Path, required=True)
    parser.add_argument('--helper-source', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--example', type=Path, required=True)
    parser.add_argument('--example-sha256', required=True)
    parser.add_argument('--compiled-revision', required=True)
    parser.add_argument('--workdir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    assert Path.cwd().resolve() == REPOSITORY_ROOT, 'Run from repository root'
    for name in ['reference_report', 'transition_audit', 'helper_dir', 'helper_source',
                 'jar', 'example', 'workdir', 'report']:
        setattr(args, name, relative_path(getattr(args, name)))
    assert not args.report.exists()
    assert not args.workdir.exists()
    assert pin(args.jar)['sha256'] == JAR_SHA256
    assert pin(args.example)['sha256'] == args.example_sha256
    assert len(args.compiled_revision) == 40
    assert all(character in '0123456789abcdef' for character in args.compiled_revision)
    assert (args.helper_dir / GETTER_CLASS_FILE).is_file()
    report = json.loads(args.reference_report.read_text())
    audit = json.loads(args.transition_audit.read_text())
    assert pin(args.reference_report) == audit['source_report']
    assert pin(args.helper_source)['sha256'] == report['helper']['sha256']
    assert report['references'] == audit['references'] == EXPECTED_CONTROLS
    originals = {(item['family'], item['profile']): item for item in report['originals']}
    input_pins = {}
    for original in originals.values():
        verify(original['input'])
        input_pins[original['input']['path']] = original['input']
    for case in report['results']:
        for field in ['source', 'reference', 'request', 'getter']:
            verify(case[field])
        input_pins[case['reference']['path']] = case['reference']
    before_files = list(input_pins.values())
    args.workdir.mkdir(parents=True, exist_ok=False)
    commands, results = [], []

    for index, case in enumerate(report['results']):
        label = f"{index:02}-{case['profile']}-{case['table']}-{case['selection']}"
        source = Path(case['source']['path'])
        before = source.read_bytes()
        original_group = originals[(case['family'], case['profile'])]
        original = original_group['tables'][case['table']]
        selected = set(case['selected_ids'])
        ordered = sorted(original['rows'].items(),
                         key=lambda item: (item[1]['page'], item[1]['slot']))
        request = args.workdir / (label + '.tsv')
        with request.open('x') as output:
            for ordinal, (identifier, row) in enumerate(ordered):
                if int(identifier) in selected:
                    reference = (row['page'] << wire.REFERENCE_SLOT_BITS) | row['slot']
                    payload = wire.row(before, reference)
                    output.write(f'{ordinal}\t{payload.hex()}\n')
        target = args.workdir / (label + '.accdb')
        argv = [str(args.example), str(source), str(target), case['table'],
                '--delete-file', str(request)]
        candidate = run(argv, args.workdir / (label + '.stdout'),
                        args.workdir / (label + '.stderr'))
        command_index = len(commands)
        commands.append(candidate)
        result = {
            'family': case['family'], 'profile': case['profile'], 'table': case['table'],
            'selection': case['selection'], 'expected': 'positive_candidate',
            'selected_ids': case['selected_ids'], 'command': command_index,
            'request': pin(request), 'agreed': False,
        }
        try:
            assert candidate['exit_code'] == 0, 'positive target refused'
            assert target.is_file()
            stdout = Path(candidate['stdout']['path']).read_text().strip()
            assert stdout == f'candidate_deleted_rows={len(selected)}'
            getter = run(oracle_command(args, target, original_group['tables']),
                         args.workdir / (label + '.getters.tsv'),
                         args.workdir / (label + '.getters.stderr'))
            result['getter_command'] = len(commands)
            commands.append(getter)
            assert getter['exit_code'] == 0
            after = target.read_bytes()
            actual = parse_getters(Path(getter['stdout']['path']), after)
            reference_raw = Path(case['reference']['path']).read_bytes()
            public_reference = parse_getters(Path(case['getter']['path']), reference_raw)
            semantic_check(original, actual[case['table']], public_reference[case['table']], selected)
            for other in original_group['tables']:
                if other != case['table']:
                    assert canonical(actual[other]) == canonical(original_group['tables'][other])
            preservation = atomic_check(before, after, original, actual[case['table']], selected)
            result.update({
                'agreed': True, 'candidate': pin(target), 'getter': getter['stdout'],
                'deleted_rows': len(selected), 'remaining_rows': actual[case['table']]['count'],
                'full_retained_encoded_keys_and_eligible_populations_match': True,
                'atomic_preservation': preservation,
            })
        except (AssertionError, ValueError, KeyError) as error:
            result.update({'failure': str(error), 'traceback': traceback.format_exc(),
                           'candidate_exists': target.exists()})
            if target.exists():
                result['candidate'] = pin(target)
        results.append(result)
        for record in before_files:
            verify(record)
        print(label, 'agreed', result['agreed'], 'exit', candidate['exit_code'], flush=True)

    output = {
        'scope': 'Fifty positive atomic candidate controls; independent public references, no native Access qualification',
        'example': pin(args.example), 'compiled_revision': args.compiled_revision,
        'reference_report': pin(args.reference_report), 'transition_audit': pin(args.transition_audit),
        'helper': pin(args.helper_source), 'helper_class': pin(args.helper_dir / GETTER_CLASS_FILE),
        'jar': pin(args.jar), 'inputs': before_files, 'commands': commands, 'results': results,
        'controls': len(results), 'disagreements': sum(not item['agreed'] for item in results),
        'originals_references_unchanged': True,
        'tool_sources': [pin(path) for path in sorted(Path(__file__).parent.glob('*.py'))],
    }
    fresh_report(args.report, output)
    print('controls', len(results), 'disagreements', output['disagreements'], flush=True)
    if output['disagreements']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
