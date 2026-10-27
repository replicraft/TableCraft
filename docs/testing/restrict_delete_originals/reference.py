"""Independent public-API RESTRICT reference actions on NEW original identities."""

import argparse
from fractions import Fraction
import json
from pathlib import Path
import subprocess
import uuid

from getters import parse_getters
from support import JAR_SHA256, REPOSITORY_ROOT, canonical, fresh_report, oracle_command, pin

KINDS = ['BYTE', 'I16', 'I32', 'I64', 'CURRENCY', 'GUID', 'DECIMAL']
PROFILES = ['V2016', 'V2019']
TIMEOUT_SECONDS = 120
FORMAT_CODE_OFFSET = 20
HEADER_RESERVED_RANGE = slice(21, 24)
ENCODING_KEY_RANGE = slice(62, 66)
PLAIN_ENCODING_KEY = bytes.fromhex('fb8abc4e')
PROFILE_CODES = {'V2016': 5, 'V2019': 6}


def value(encoded, column):
    if encoded == 'NULL':
        return None
    kind = column[1]
    if kind == 'BYTE':
        return int(encoded.split(':', 1)[1]) & 255
    if kind in ['INT', 'LONG', 'BIG_INT']:
        return int(encoded.split(':', 1)[1])
    if kind in ['MONEY', 'NUMERIC']:
        _, coefficient, scale = encoded.split(':')
        return Fraction(int(coefficient), 10 ** int(scale))
    if kind == 'GUID':
        text = bytes.fromhex(encoded.split(':', 1)[1]).decode('utf-16-be')
        return uuid.UUID(text.strip('{}')).bytes
    raise AssertionError(('unqualified reference carrier', kind))


def violations(snapshot, target=None, removed=()):
    tables = snapshot['tables']
    failures = []
    for name, relation in snapshot['relationships'].items():
        parent = tables[relation['parent']]
        child = tables[relation['child']]
        parent_keys = []
        for identifier, row in parent['rows'].items():
            if relation['parent'] == target and identifier in removed:
                continue
            parent_keys.append(tuple(value(row['values'][column], parent['columns'][column])
                                     for column, other in relation['columns']))
        for identifier, row in child['rows'].items():
            if relation['child'] == target and identifier in removed:
                continue
            key = tuple(value(row['values'][column], child['columns'][column])
                        for other, column in relation['columns'])
            if all(component is None for component in key):
                continue
            if key not in parent_keys:
                failures.append((name, identifier))
    return failures


def tables_list():
    names = [kind + suffix for kind in KINDS for suffix in ['Parents', 'Children']]
    return names + ['I32SecondChildren', 'IntegerTupleParents', 'IntegerTupleChildren',
                    'DecimalTupleParents', 'DecimalTupleChildren']


def plan():
    result = []
    for kind in KINDS:
        result.extend([(kind + 'Children', [1], True, None, 'child-nonnull'),
                       (kind + 'Parents', [4], True, None, 'parent-unreferenced-30'),
                       (kind + 'Parents', [2], False, None, 'parent-referenced-10')])
    for kind in ['I64', 'CURRENCY', 'DECIMAL']:
        result.append((kind + 'Parents', [8], True, None, 'parent-adjacent-unreferenced'))
    result.extend([('I32Children', [4], True, None, 'child-null'),
                   ('I32Parents', [6], True, None, 'parent-null')])
    for prefix in ['IntegerTuple', 'DecimalTuple']:
        result.extend([(prefix + 'Children', [3], True, None, 'child-all-null'),
                       (prefix + 'Parents', [3], True, None, 'parent-cross-components-unreferenced'),
                       (prefix + 'Parents', [4], False, None, 'parent-partial-null-referenced'),
                       (prefix + 'Parents', [6, 7], True, None, 'parents-all-null'),
                       (prefix + 'Children', [4], True, prefix, 'history-clear-partial-child'),
                       (prefix + 'Parents', [4], True, prefix, 'history-delete-partial-parent')])
    result.extend([('I32Children', [1, 2], True, 'multiple-children', 'history-clear-first-child-table'),
                   ('I32Parents', [2], False, 'multiple-children', 'history-second-child-still-restricts'),
                   ('I32SecondChildren', [1], True, 'multiple-children', 'history-clear-second-child-table'),
                   ('I32Parents', [2], True, 'multiple-children', 'history-parent-after-all-children')])
    assert len(result) == 42
    return result


def run(argv, folder, label):
    stdout, stderr = folder / (label + '.stdout'), folder / (label + '.stderr')
    with stdout.open('xb') as out, stderr.open('xb') as err:
        result = subprocess.run(argv, stdout=out, stderr=err, timeout=TIMEOUT_SECONDS)
    return {'argv': argv, 'exit_code': result.returncode,
            'stdout': pin(stdout), 'stderr': pin(stderr)}


def normalized(snapshot):
    # Tuple-key type metadata is separately pinned; schema and values are JSON-safe.
    return {'tables': snapshot['tables'], 'relationships': snapshot['relationships']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', action='append', nargs=2, required=True)
    parser.add_argument('--baseline-dir', type=Path, required=True)
    parser.add_argument('--helper-dir', type=Path, required=True)
    parser.add_argument('--helper-source', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--workdir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    assert Path.cwd().resolve() == REPOSITORY_ROOT
    for name in ['baseline_dir', 'helper_dir', 'helper_source', 'jar', 'workdir', 'report']:
        path = getattr(args, name)
        setattr(args, name, Path(path.resolve().relative_to(REPOSITORY_ROOT).as_posix()))
    args.input = [(profile, str(Path(path).resolve().relative_to(REPOSITORY_ROOT)))
                  for profile, path in args.input]
    assert pin(args.jar)['sha256'] == JAR_SHA256
    assert not args.workdir.exists() and not args.report.exists()
    assert len(args.input) == len(PROFILE_CODES), 'Exactly one input per profile is required'
    sources = {profile: Path(path) for profile, path in args.input}
    assert set(sources) == set(PROFILES)
    source_pins = [pin(path) for path in sources.values()]
    args.workdir.mkdir(parents=True, exist_ok=False)
    commands, originals, results = [], [], []
    for profile in PROFILES:
        source = sources[profile]
        source_bytes = source.read_bytes()
        assert source_bytes[FORMAT_CODE_OFFSET] == PROFILE_CODES[profile]
        assert source_bytes[HEADER_RESERVED_RANGE] == bytes(HEADER_RESERVED_RANGE.stop - HEADER_RESERVED_RANGE.start)
        assert source_bytes[ENCODING_KEY_RANGE] == PLAIN_ENCODING_KEY
        original = parse_getters(args.baseline_dir / f'{profile}-before-getters.tsv', source_bytes)
        assert not violations(original)
        originals.append({'profile': profile, 'input': pin(source), 'snapshot': normalized(original)})
        histories = {}
        for table, identifiers, expected, history, label in plan():
            state = histories.get(history, (source, original, None)) if history else (source, original, None)
            before_path, before, prior_success = state
            predicted = not violations(before, table, set(identifiers))
            assert predicted == expected, ('independent model disagrees with frozen action plan', table, label)
            index = len(results)
            name = f'{index:02}-{profile}-{table}-{label}'
            request = args.workdir / (name + '.ids.tsv')
            request.write_text(''.join(f'{identifier}\n' for identifier in identifiers))
            target = args.workdir / (name + '.accdb')
            base = oracle_command(args, before_path, tables_list())[:4]
            command = run(base + ['reference', str(before_path), str(target), table, str(request)], args.workdir, name)
            command_index = len(commands)
            commands.append(command)
            actual_success = command['exit_code'] == 0 and Path(command['stdout']['path']).read_text().startswith('REFERENCE\t')
            actual_rejection = command['exit_code'] == 0 and Path(command['stdout']['path']).read_text().startswith('REJECTED_CONSTRAINT_UNCHANGED\t')
            result = {'profile': profile, 'table': table, 'selected_ids': identifiers,
                      'action': label, 'history': history, 'candidate_source_case': prior_success,
                      'source': pin(before_path), 'reference': pin(target), 'request': pin(request),
                      'command': command_index, 'expected_success': expected,
                      'actual_success': actual_success, 'actual_constraint_rejection': actual_rejection,
                      'agreed': actual_success if expected else actual_rejection}
            getter = run(oracle_command(args, target, tables_list()), args.workdir, name + '-getter')
            result['getter_command'] = len(commands)
            commands.append(getter)
            assert getter['exit_code'] == 0, getter
            after = parse_getters(Path(getter['stdout']['path']), target.read_bytes())
            result['getter'] = getter['stdout']
            assert not violations(after)
            assert canonical(after['relationships']) == canonical(before['relationships'])
            if actual_rejection:
                assert target.read_bytes() == before_path.read_bytes()
                assert canonical(normalized(after)) == canonical(normalized(before))
                result['rejected_copy_byte_identical'] = True
            elif actual_success:
                for name_table, old in before['tables'].items():
                    current = after['tables'][name_table]
                    rows = {key: row for key, row in old['rows'].items()
                            if name_table != table or key not in identifiers}
                    assert canonical(current['rows']) == canonical(rows)
                    assert current['count'] == old['count'] - (len(identifiers) if name_table == table else 0)
                    for name_index, index_old in old['indexes'].items():
                        new_index = current['indexes'][name_index]
                        assert new_index['historical'] == index_old['historical']
                        assert new_index['order'] == [key for key in index_old['order']
                                                     if name_table != table or key not in identifiers]
                result['remaining_rows'] = after['tables'][table]['count']
            results.append(result)
            for path, source_pin in zip(sources.values(), source_pins):
                assert pin(path) == source_pin
            print(index, profile, table, label, 'expected', expected, 'actual', actual_success,
                  'rejected_unchanged', actual_rejection, 'agreed', result['agreed'], flush=True)
            if history:
                histories[history] = (target, after, index if actual_success else prior_success)
    output = {'scope': 'NEW original identity; individual public API actions with enforced foreign keys, not SQL statement timing or native Access qualification',
              'helper': pin(args.helper_source), 'runner': pin(Path(__file__)), 'jar': pin(args.jar),
              'originals': originals, 'commands': commands, 'results': results,
              'controls': len(results), 'planned_controls': 84,
              'disagreements': sum(not result['agreed'] for result in results),
              'original_inputs_unchanged': True}
    fresh_report(args.report, output)
    print('controls', len(results), 'disagreements', output['disagreements'], flush=True)
    if output['disagreements']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
