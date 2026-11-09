"""Fresh parent-overflow source reproduction; no overflowing UPDATE actions.

This maintained public wrapper is distinct from the archived first execution.
Use verify_existing.py to check accepted artifacts without native/JVM calls.
"""
import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
import traceback
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'docs/testing'))
sys.path.insert(0, str(HERE))
from slot_preview_originals import common
import delete_reference_maps as maps
import wire

JAR = Path('.cache/tablecraft-fixtures/jackcess-5.0.4.jar')
JAVA_SOURCES = (HERE / 'BoundedFileChannel.java', HERE / 'ParentSplitFixture.java')
FROZEN_EXAMPLE_SHA256 = '8245c35b6b533dbdc8f6dec5fed74aab0c0fdaf0da73e5df529dd3053eae4709'
COMPILED_REVISION = '0c0dc6d904bd112236a383c74b04192ee6c5f90f'
PROFILES = ('V2016', 'V2019')
PROCESS_TIMEOUT_SECONDS = 1200
JAVA_HEAP = '-Xmx512m'
MAX_ROWS = 149_999
MAX_FILE_BYTES = 64 * 1024 * 1024
MAX_FINAL_ROWS = 16_384
GOAL_FINAL_ROWS = 1536
MAX_MAPPED_PAGES = 4096
SELECTED_TABLE = 'SparseInt32'
SELECTED_COLUMN = 'Key'


def pin(path):
    return common.pin(path)


class Commands:
    def __init__(self, directory):
        self.directory = directory
        self.records = []

    def run(self, arguments, label):
        stdout = self.directory / (label + '.stdout')
        stderr = self.directory / (label + '.stderr')
        with stdout.open('xb') as out, stderr.open('xb') as err:
            process = subprocess.Popen(arguments, cwd=ROOT, stdout=out, stderr=err)
            deadline = time.monotonic() + PROCESS_TIMEOUT_SECONDS
            status = process.poll()
            while status is None and time.monotonic() < deadline:
                time.sleep(0.05)
                status = process.poll()
            timed_out = status is None
            if timed_out:
                process.kill()
                status = process.poll()
        result = {'argv': list(map(str, arguments)), 'status': status,
                  'timed_out': timed_out, 'timeout_seconds': PROCESS_TIMEOUT_SECONDS,
                  'stdout': pin(stdout), 'stderr': pin(stderr)}
        self.records.append(result)
        with (self.directory / 'commands.jsonl').open('a') as output:
            output.write(json.dumps(result) + '\n')
        return result


def checked(result):
    assert result['status'] == 0 and not result['timed_out'], result
    assert not common.path(result['stderr']['path']).read_bytes(), result


def java(args, classes, *arguments):
    return ['java', JAVA_HEAP, '-Dstdout.encoding=UTF-8', '-cp',
            os.pathsep.join([str(common.path(args.jar)), str(classes)]),
            'ParentSplitFixture', *map(str, arguments)]


def verify_build(args):
    assert pin(args.jar)['sha256'] == common.JAR_SHA256
    build = json.loads(common.path(args.build_receipt).read_text())
    assert build['compiled_revision'] == COMPILED_REVISION and build['exit_code'] == 0
    assert pin(args.admission_example) == build['frozen_example']
    assert build['frozen_example']['sha256'] == FROZEN_EXAMPLE_SHA256
    for source in build['source_pins']:
        body = subprocess.run(['git', 'show', COMPILED_REVISION + ':' + source['path']], cwd=ROOT,
                              check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout
        assert len(body) == source['bytes']
        assert hashlib.sha256(body).hexdigest() == source['sha256']
    return {'receipt': pin(args.build_receipt), 'artifact': pin(args.admission_example),
            'compiled_revision': COMPILED_REVISION, 'historical_git_source_count': len(build['source_pins'])}


def describe(args, path, label, classes, commands, chosen, sparse):
    expected = pin(path)
    command = commands.run(java(args, classes, 'describe', path), label)
    checked(command)
    text = common.path(command['stdout']['path']).read_text()
    tables, map_text = wire.parse_description(text)
    raw = common.path(path).read_bytes()
    observed_root = tables[SELECTED_TABLE]['indexes']['PrimaryKey']['root']
    observed_nodes, observed_leaves, observed_entries = wire.tree(
        raw, observed_root, tables[SELECTED_TABLE]['owner'],
    )
    observation = {
        'file': expected, 'command': command, 'root': observed_root,
        'height': observed_nodes[observed_root]['height'],
        'pages': sorted(observed_nodes), 'leaves': observed_leaves,
        'leaf_populations': {str(number): len(observed_nodes[number]['entries'])
                             for number in observed_leaves},
        'typed_rows': tables[SELECTED_TABLE]['count'],
        'full_key_row_sha256': hashlib.sha256(b''.join(observed_entries)).hexdigest(),
    }
    observation_path = commands.directory / (label + '.observation.json')
    observation_path.write_text(json.dumps(observation, indent=2) + '\n')
    map_metadata = maps.parse(map_text)
    map_proof = {name: maps.decode_map(raw, metadata, wire.PAGE_BYTES, len(raw) // wire.PAGE_BYTES)
                 for name, metadata in map_metadata['maps'].items()}
    proof = wire.certify(raw, tables, chosen, sparse)
    native_order = wire.native_order(raw, map_proof['TableOwned']['file_members'], tables[SELECTED_TABLE])
    proof['selected_row']['native_ordinal'] = native_order[proof['selected_value']]
    proof['selected_row']['public_ordinal'] = proof['selected_row']['ordinal']
    assert len(map_proof['TableOwned']['file_members']) <= MAX_MAPPED_PAGES
    assert len(map_proof['IndexOwned']['file_members']) <= MAX_MAPPED_PAGES
    assert sorted(map_proof['IndexOwned']['file_members']) == sorted(map(int, proof['nodes']))
    assert pin(path) == expected
    proof_path = commands.directory / (label + '.proof.json')
    proof_path.write_text(json.dumps({'file': expected, 'command': command, 'proof': proof,
                                    'maps': map_proof, 'map_metadata': map_metadata}, indent=2) + '\n')
    return proof, map_proof, {'proof': pin(proof_path), 'command': command, 'file': expected,
                             'observation': pin(observation_path)}


def execute(args, build):
    manifest = json.loads(common.path(args.freeze).read_text())
    for source in manifest['sources']:
        assert pin(source['path']) == source
    directory = common.fresh_work(args.workdir)
    classes = directory / 'classes'
    classes.mkdir()
    commands = Commands(directory)
    archived = []
    archive = directory / 'precall-sources'
    archive.mkdir()
    for position, source in enumerate(manifest['sources']):
        output = archive / (str(position) + '-' + Path(source['path']).name)
        output.write_bytes(common.path(source['path']).read_bytes())
        archived.append(pin(output))
    precall = directory / 'before-first-command.json'
    precall.write_text(json.dumps({'freeze': pin(args.freeze), 'build': build,
                                  'source_archives': archived}, indent=2) + '\n')
    report = {'scope': 'New public generation/sparse histories and two K-to-K complete admission calls only; no K-to-MAX or public UPDATE',
              'precall': pin(precall), 'cases': [], 'completed': 0}
    report_path = directory / 'report.json'
    compile_command = commands.run(['javac', '-cp', str(common.path(args.jar)), '-d', str(classes),
                                    *map(str, JAVA_SOURCES)], 'compile')
    report['compile'] = compile_command
    report['commands'] = commands.records
    report_path.write_text(json.dumps(report, indent=2) + '\n')
    checked(compile_command)
    report['classes'] = [pin(path) for path in sorted(classes.glob('*.class'))]
    for profile in PROFILES:
        item = {'profile': profile, 'complete': False}
        report['cases'].append(item)
        large = directory / (profile + '-larger.accdb')
        sparse = directory / (profile + '-sparse.accdb')
        selection = directory / (profile + '-selection.tsv')
        try:
            generated = commands.run(java(args, classes, 'generate', large, profile, selection),
                                     profile + '-generate')
            item['generation'] = generated
            item['larger_file'] = pin(large) if large.exists() else None
            item['selection'] = pin(selection) if selection.exists() else None
            checked(generated)
            chosen = wire.selection(selection.read_text())
            assert chosen['SOURCE_SHA256'] == pin(large)['sha256']
            assert int(chosen['INITIAL_ROWS']) <= MAX_ROWS
            assert len(chosen['keep']) <= MAX_FINAL_ROWS
            full, _, full_capture = describe(args, large, profile + '-larger-description',
                                             classes, commands, chosen, False)
            item['larger_capture'] = full_capture
            # Freeze the exact discovered selectors before pruning. The rule was
            # fixed before generation: first gate, maxima plus full rightmost leaf.
            action = directory / (profile + '-before-pruning.json')
            action.write_text(json.dumps({'source': pin(large), 'selection': pin(selection),
                                          'selected_value': int(chosen['SELECTED']),
                                          'keep_count': len(chosen['keep']),
                                          'initial_geometry_proof': full_capture['proof']}, indent=2) + '\n')
            item['before_pruning'] = pin(action)
            pruned = commands.run(java(args, classes, 'sparse', large, sparse, selection),
                                  profile + '-prune')
            item['pruning'] = pruned
            item['sparse_file'] = pin(sparse) if sparse.exists() else None
            checked(pruned)
            final, final_maps, sparse_capture = describe(
                args, sparse, profile + '-sparse-description', classes, commands, chosen, True,
            )
            item['sparse_capture'] = sparse_capture
            item['history'] = wire.compare_history(full, final)
            assert pin(large) == item['larger_file']
            eof = sparse.stat().st_size // wire.PAGE_BYTES
            item['final_geometry'] = final
            item['metadata_goals'] = {
                'live_1536': final['live_rows'] <= GOAL_FINAL_ROWS,
                'data_owned_768': len(final_maps['TableOwned']['file_members']) <= 768,
                'index_owned_640': len(final_maps['IndexOwned']['file_members']) <= 640,
                'file_64MiB': sparse.stat().st_size <= MAX_FILE_BYTES,
            }
            item['future_eof'] = eof
            proof_body = json.loads(common.path(sparse_capture['proof']['path']).read_text())
            future = {}
            for name in ('JackcessGlobalFree', 'IndexOwned'):
                metadata = proof_body['map_metadata']['maps'][name]
                future[name] = [wire.future_cell(sparse.read_bytes(), metadata, eof + index)
                                for index in range(3)]
            assert all(cell['set'] for cell in future['JackcessGlobalFree'])
            assert all(not cell['set'] for cell in future['IndexOwned'])
            item['future_cells_for_three_pages'] = future
            row = final['selected_row']
            admission = directory / (profile + '-admission-noop.accdb')
            before_admission = directory / (profile + '-before-admission.json')
            before_admission.write_text(json.dumps({
                'source': pin(sparse), 'selected_row': row,
                'expected_and_replacement': final['selected_value'],
                'artifact': build['artifact'],
            }, indent=2) + '\n')
            item['before_admission'] = pin(before_admission)
            command = commands.run([
                str(common.path(args.admission_example)), str(sparse), str(admission),
                SELECTED_TABLE, SELECTED_COLUMN, str(row['native_ordinal']),
                str(final['selected_value']), str(final['selected_value']),
            ], profile + '-native-admission-noop')
            item['native_admission'] = command
            item['native_admission_error'] = common.path(command['stderr']['path']).read_text()
            assert command['status'] == 0 and not command['timed_out'], 'Complete existing admission refused'
            assert admission.read_bytes() == sparse.read_bytes(), 'Admission output changed source bytes'
            item['byte_identical_admission_output'] = pin(admission)
            assert pin(sparse) == item['sparse_file']
            assert pin(large) == item['larger_file']
            item['complete'] = True
            report['completed'] += 1
        except Exception as error:
            item['failure'] = {'class': type(error).__name__, 'message': str(error),
                               'traceback': traceback.format_exc()}
        finally:
            observations = [directory / (profile + suffix + '.observation.json')
                            for suffix in ('-larger-description', '-sparse-description')]
            if all(path.exists() for path in observations):
                initial, final_observed = [json.loads(path.read_text()) for path in observations]
                item['observed_sparse_topology'] = {
                    'observation_pins': [pin(path) for path in observations],
                    'root_before_after': [initial['root'], final_observed['root']],
                    'height_before_after': [initial['height'], final_observed['height']],
                    'missing_original_pages': sorted(set(initial['pages']) - set(final_observed['pages'])),
                    'new_pages': sorted(set(final_observed['pages']) - set(initial['pages'])),
                    'missing_original_leaves': sorted(set(initial['leaves']) - set(final_observed['leaves'])),
                    'leaf_populations_after': final_observed['leaf_populations'],
                }
            report['commands'] = commands.records
            report['command_count'] = len(commands.records)
            report_path.write_text(json.dumps(report, indent=2) + '\n')
    for source in manifest['sources']:
        assert pin(source['path']) == source
    report['immutable_tools_unchanged'] = True
    report_path.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'report': pin(report_path), 'complete_profiles': report['completed']}))
    return 0 if report['completed'] == len(PROFILES) else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, default=JAR)
    parser.add_argument('--admission-example', type=Path,
                        default=Path('.cache/tablecraft-bin/ace16-root-split-20261027'))
    parser.add_argument('--build-receipt', type=Path,
                        default=Path('docs/testing/results/ace16-root-split-example-build-2026-10-27.json'))
    parser.add_argument('--freeze', type=Path)
    parser.add_argument('--freeze-current', type=Path,
                        help='Write a fresh current-tool source manifest; no JVM/native calls')
    parser.add_argument('--workdir', type=Path)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    build = verify_build(args)
    if args.freeze_current is not None:
        assert not args.execute
        destination = common.path(args.freeze_current)
        destination.relative_to(ROOT / '.cache')
        sources = [*JAVA_SOURCES, HERE / 'wire.py', HERE / 'capture.py',
                   HERE / 'selfcheck.py', HERE / 'verify_existing.py',
                   ROOT / 'docs/testing/slot_preview_originals/common.py',
                   ROOT / 'docs/testing/delete_reference_maps.py']
        destination.parent.mkdir(parents=True, exist_ok=True)
        with destination.open('x') as output:
            json.dump({'sources': [pin(path) for path in sources],
                       'scope': 'Current public reproduction sources, not historical execution',
                       'limits': {'aggregate_user_rows': MAX_ROWS + 1,
                                  'bytes_per_file': MAX_FILE_BYTES,
                                  'live_rows_after_pruning': MAX_FINAL_ROWS}}, output, indent=2)
            output.write('\n')
        print(json.dumps(pin(destination)))
        return 0
    if not args.execute:
        print('Historical admission artifact/source pins verified; no JVM/native calls or database generation.')
        return 0
    if args.workdir is None or args.freeze is None:
        parser.error('--execute requires a fresh --workdir and --freeze current-source manifest')
    return execute(args, build)


if __name__ == '__main__':
    raise SystemExit(main())
