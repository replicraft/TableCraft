"""Explicitly gated fresh internal-parent source experiment; no native calls."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import traceback

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'docs/testing'))
sys.path.insert(0, str(HERE))
import certificate
import delete_reference_maps as maps
from parent_index_split_originals import wire

TIMEOUT_SECONDS = 1200
JAVA_HEAP = '-Xmx512m'
JAR = ROOT / '.cache/tablecraft-fixtures/jackcess-5.0.4.jar'
JAR_HASH = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
MAX_TARGET_ROWS = 149999
MAX_FINAL_ROWS = 16384
MAX_FILE_BYTES = 64 * 1024 * 1024


def pin(path):
    path = Path(path)
    if not path.is_absolute():
        path = ROOT / path
    data = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'bytes': len(data),
            'sha256': hashlib.sha256(data).hexdigest()}


def checked(command):
    assert command['status'] == 0 and not command['timed_out'], command
    assert command['stderr']['bytes'] == 0, command


class Commands:
    def __init__(self, directory):
        self.directory = directory
        self.records = []

    def run(self, argv, label):
        stdout = self.directory / (label + '.stdout')
        stderr = self.directory / (label + '.stderr')
        with stdout.open('xb') as output, stderr.open('xb') as errors:
            process = subprocess.Popen(argv, cwd=ROOT, stdout=output, stderr=errors)
            timed_out = False
            try:
                status = process.wait(timeout=TIMEOUT_SECONDS)
            except subprocess.TimeoutExpired:
                timed_out = True
                process.kill()
                status = process.wait(timeout=5)
        record = {
            'argv': list(map(str, argv)), 'status': status, 'timed_out': timed_out,
            'timeout_seconds': TIMEOUT_SECONDS, 'stdout': pin(stdout), 'stderr': pin(stderr),
        }
        self.records.append(record)
        with (self.directory / 'commands.jsonl').open('a') as output:
            output.write(json.dumps(record) + '\n')
        return record


def java(classes, *arguments):
    return ['java', JAVA_HEAP, '-Dstdout.encoding=UTF-8', '-cp',
            os.pathsep.join([str(JAR), str(classes)]),
            'InternalParentSplitFixture', *map(str, arguments)]


def describe(source, label, classes, commands, selection, sparse):
    original = pin(source)
    command = commands.run(java(classes, 'describe', source), label)
    checked(command)
    tables, map_text = wire.parse_description((ROOT / command['stdout']['path']).read_text())
    raw = source.read_bytes()
    proof = certificate.certify(raw, tables, selection, sparse)
    metadata = maps.parse(map_text)
    map_proofs = {}
    for role, description in metadata['maps'].items():
        map_proofs[role] = maps.decode_map(raw, description, wire.PAGE_BYTES, len(raw) // wire.PAGE_BYTES)
    assert sorted(map_proofs['IndexOwned']['file_members']) == sorted(map(int, proof['nodes']))
    order = wire.native_order(raw, map_proofs['TableOwned']['file_members'], tables['SparseInt32'])
    proof['selected_row']['native_ordinal'] = order[proof['selected_value']]
    assert pin(source) == original
    path = commands.directory / (label + '.proof.json')
    path.write_text(json.dumps({'file': original, 'proof': proof, 'maps': map_proofs,
                                'map_metadata': metadata, 'command': command}, indent=2) + '\n')
    return proof, {'file': original, 'proof': pin(path), 'command': command}


def execute(args, freeze):
    directory = ROOT / args.workdir
    directory.mkdir(exist_ok=False)
    classes = directory / 'classes'
    classes.mkdir()
    archive = directory / 'precall-sources'
    archive.mkdir()
    archives = []
    for position, source in enumerate(freeze['sources']):
        target = archive / (str(position) + '-' + Path(source['path']).name)
        target.write_bytes((ROOT / source['path']).read_bytes())
        archives.append({'source': source, 'archive': pin(target)})
    before = directory / 'before-first-command.json'
    before.write_text(json.dumps({'freeze': pin(args.freeze), 'archives': archives}, indent=2) + '\n')
    commands = Commands(directory)
    report = {'scope': 'New source generation/pruning only; no MAX UPDATE/native action',
              'before_first_command': pin(before), 'cases': [], 'completed_profiles': 0}
    path = directory / 'report.json'

    def save():
        report['commands'] = commands.records
        report['process_count'] = len(commands.records)
        path.write_text(json.dumps(report, indent=2) + '\n')

    try:
        command = commands.run([
            'javac', '-cp', str(JAR), '-d', str(classes),
            str(ROOT / 'docs/testing/parent_index_split_originals/BoundedFileChannel.java'),
            str(HERE / 'InternalParentSplitFixture.java'),
        ], 'compile')
        report['compile'] = command
        save()
        checked(command)
        report['classes'] = [pin(item) for item in sorted(classes.glob('*.class'))]
        for profile in ('V2016', 'V2019'):
            case = {'profile': profile, 'complete': False}
            report['cases'].append(case)
            large = directory / (profile + '-larger.accdb')
            sparse = directory / (profile + '-sparse.accdb')
            selected = directory / (profile + '-selection.tsv')
            try:
                generated = commands.run(java(classes, 'generate', large, profile, selected), profile + '-generate')
                case['generation'] = generated
                case['larger_file'] = pin(large) if large.exists() else None
                case['selection'] = pin(selected) if selected.exists() else None
                save()
                checked(generated)
                selection = wire.selection(selected.read_text())
                assert selection['SOURCE_SHA256'] == pin(large)['sha256']
                assert int(selection['INITIAL_ROWS']) <= MAX_TARGET_ROWS
                assert len(selection['keep']) <= MAX_FINAL_ROWS
                assert large.stat().st_size <= MAX_FILE_BYTES
                full, receipt = describe(large, profile + '-larger-description', classes, commands, selection, False)
                case['larger_capture'] = receipt
                case['before_pruning_selection'] = pin(selected)
                pruned = commands.run(java(classes, 'sparse', large, sparse, selected), profile + '-prune')
                case['pruning'] = pruned
                case['sparse_file'] = pin(sparse) if sparse.exists() else None
                save()
                checked(pruned)
                final, receipt = describe(sparse, profile + '-sparse-description', classes, commands, selection, True)
                case['sparse_capture'] = receipt
                case['history'] = certificate.compare_history(full, final)
                assert pin(large) == case['larger_file']
                assert pin(selected) == case['before_pruning_selection']
                assert sparse.stat().st_size <= MAX_FILE_BYTES
                case['complete'] = True
                report['completed_profiles'] += 1
            except Exception as error:
                case['failure'] = {'class': type(error).__name__, 'message': str(error),
                                   'traceback': traceback.format_exc()}
            finally:
                if large.exists():
                    case['larger_after'] = pin(large)
                if sparse.exists():
                    case['sparse_after'] = pin(sparse)
                save()
    except Exception as error:
        report['fatal'] = {'class': type(error).__name__, 'message': str(error),
                           'traceback': traceback.format_exc()}
    finally:
        for source in freeze['sources']:
            assert pin(source['path']) == source
        report['immutable_tools_unchanged'] = True
        save()
    print(json.dumps({'report': pin(path), 'completed_profiles': report['completed_profiles'],
                      'process_count': report['process_count']}))
    return 0 if report['completed_profiles'] == 2 else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--freeze', type=Path, default=HERE / 'freeze.json')
    parser.add_argument('--workdir', type=Path)
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--freeze-current', type=Path)
    args = parser.parse_args()
    if args.freeze_current is not None:
        if args.execute:
            parser.error('--freeze-current and --execute are separate operations')
        output = ROOT / args.freeze_current
        if output.exists():
            raise FileExistsError(output)
        sources = [Path(__file__), HERE / 'certificate.py', HERE / 'InternalParentSplitFixture.java',
                   ROOT / 'docs/testing/parent_index_split_originals/BoundedFileChannel.java',
                   ROOT / 'docs/testing/parent_index_split_originals/wire.py',
                   ROOT / 'docs/testing/delete_reference_maps.py']
        output.write_text(json.dumps({'status': 'Fresh public-source reproduction manifest',
                                     'sources': [pin(path) for path in sources],
                                     'jar': pin(JAR)}, indent=2) + '\n')
        print('Current-source manifest only; no database actions.')
        return 0
    freeze = json.loads((ROOT / args.freeze).read_text())
    for source in freeze['sources']:
        assert pin(source['path']) == source
    assert pin(JAR)['sha256'] == JAR_HASH
    if not args.execute:
        print('Static sources and reviewed JAR verified; no compilation/JVM/native actions.')
        return 0
    if args.workdir is None:
        parser.error('--execute requires a fresh workdir and explicit root GO')
    return execute(args, freeze)


if __name__ == '__main__':
    raise SystemExit(main())
