"""Saved engineering candidates plus independent fresh readbacks; no default child."""
import argparse
import ast
import hashlib
import json
from pathlib import Path, PurePosixPath
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[3]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'docs/testing'))
from scripts.compat_artifacts import ArtifactStore
from ace16_insert_multiple_indexes import model, reference, verify

EXPECTED = '.cache/ace16-insert-multiple-index-expected-second-20261028'
NATIVE = '.cache/ace16-insert-multiple-index-candidates-next-20261028'
NATIVE_WORK = '.cache/ace16-insert-multiple-index-candidates-first-20261028'
GETTERS = '.cache/ace16-insert-multiple-index-getters-next-20261028'
GETTER_WORK = '.cache/ace16-insert-multiple-index-getters-first-20261028'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
FIXED = {
    EXPECTED + '/freeze.json': '3035a90dd5cf1b4033fd6b9222f3eb0e9613edce51f901e68a6c11829c72bceb',
    NATIVE + '/protocol-freeze.json': 'e6f8eedf75828caf828738fe54a5eed105a20e20f3941952faa00488db2ae648',
    NATIVE + '/binding-first.json': 'f9b6c4ea1f088a431391bff6284343a931b386adc01dfb7403c0141544561890',
    NATIVE_WORK + '/report.json': '1ba25a485000d04a2ee3a3bf966a6e8db209ff7827d5c608ef9b3adaaf4f102b',
    GETTERS + '/controller-freeze.json': 'fd50cbd31428cf91fb343414c586d2cee6dd3e4390fe1c374964447e83a76f41',
    GETTERS + '/binding-first.json': '1aa833f2cff827f0beaa0529758a0183526e0b9a0353092d3e5b3dde8717b1af',
    GETTER_WORK + '/report.json': '9b6a87a431a675f8ce199e5de443e6d3d1f459fc342c17ffb9c0529132a5dcc6',
}


class Pins:
    def __init__(self, store):
        self.store = store
        self.items = {}
        self.occurrences = 0

    def check(self, item):
        assert self.items.setdefault(item['path'], item) == item
        assert self.store.pin(item['path']) == item, item['path']
        self.occurrences += 1
        return self.store.path(item['path'])

    def fixed(self, path, digest):
        item = self.store.pin(path)
        assert item['sha256'] == digest, path
        return self.check(item)

    def closure(self, path):
        frozen = self.store.load(path)
        for item in frozen['inputs'] + frozen.get('outputs', []):
            self.check(item)
        return frozen

    def terminal(self):
        for item in list(self.items.values()):
            self.check(item)


def command_report(store, pins, path, count, maximum):
    report = store.load(path)
    assert report['completed'] and not report.get('failure')
    assert report['after_pin_errors'] == report['artifact_pin_errors'] == []
    assert len(report['commands']) == count
    for item in report['before'] + report['artifacts']:
        pins.check(item)
    for call in report['commands']:
        assert call['status'] == 0 and call['stopped_for'] is None and call['capture_errors'] == []
        assert call['timeout_seconds'] == 30 and call['stream_limit_bytes'] == maximum
        assert call['observed_stream_bytes'] == [call['stdout']['bytes'], call['stderr']['bytes']]
        assert max(call['observed_stream_bytes']) <= maximum
        pins.check(call['stdout'])
        pins.check(call['stderr'])
    return report


def git_archive(store, pins, binding):
    archive_pin = binding['source_archive']
    pins.check(archive_pin)
    assert archive_pin['bytes'] <= 128 * 1024 * 1024
    files = []
    names = set()
    with tarfile.open(store.path(archive_pin['path'])) as archive:
        assert archive.pax_headers['comment'] == binding['revision']
        for member in archive.getmembers():
            name = PurePosixPath(member.name)
            assert not name.is_absolute() and '..' not in name.parts
            assert '\n' not in member.name and '\r' not in member.name
            assert member.name not in names
            names.add(member.name)
            assert member.isdir() or member.isfile()
            if member.isfile():
                data = archive.extractfile(member).read()
                assert len(data) == member.size
                files.append({'path': member.name, 'bytes': member.size,
                              'sha256': hashlib.sha256(data).hexdigest()})
    assert files and sum(item['bytes'] for item in files) <= 96 * 1024 * 1024
    queries = [binding['receipt_revision'] + ':' + binding['build_receipt']['path']]
    queries += [binding['revision'] + ':' + item['path'] for item in files]
    assert all('\n' not in item and '\r' not in item for item in queries)
    result = subprocess.run(['git', '--no-replace-objects', 'cat-file', '--batch'],
                            cwd=store.path('.'), input=('\n'.join(queries) + '\n').encode(),
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=True)
    assert not result.stderr and len(result.stdout) <= 128 * 1024 * 1024
    position = 0
    for item in [binding['build_receipt']] + files:
        end = result.stdout.index(b'\n', position)
        header = result.stdout[position:end].split()
        assert len(header) == 3 and header[1] == b'blob'
        size = int(header[2])
        begin = end + 1
        assert size == item['bytes']
        assert hashlib.sha256(result.stdout[begin:begin + size]).hexdigest() == item['sha256']
        assert result.stdout[begin + size:begin + size + 1] == b'\n'
        position = begin + size + 1
    assert position == len(result.stdout)
    return {'archive_files': len(files), 'receipt_blobs': 1, 'metadata_children': 1}


def replay(root, verify_git=False):
    store = ArtifactStore(root)
    pins = Pins(store)
    for path, digest in FIXED.items():
        pins.fixed(path, digest)
    expected_freeze = pins.closure(EXPECTED + '/freeze.json')
    assert expected_freeze['source_gates_before_any_image'] == 4
    assert expected_freeze['fixture_processes'] == expected_freeze['public_after_images_read_for_construction'] == 0
    pins.closure(NATIVE + '/protocol-freeze.json')
    pins.closure(GETTERS + '/controller-freeze.json')
    private = pins.check(store.pin(NATIVE + '/model.py')).read_text()
    public = pins.check(store.pin('docs/testing/ace16_insert_multiple_indexes/model.py')).read_text()
    old = 'ROOT = Path(__file__).resolve().parents[2]'
    assert private.count(old) == 1
    assert public == private.replace(old, 'ROOT = Path(__file__).resolve().parents[3]')
    definitions = lambda source: [ast.dump(node, include_attributes=False) for node in ast.parse(source).body
                                  if isinstance(node, (ast.FunctionDef, ast.ClassDef))]
    assert definitions(private) == definitions(public)
    plan = store.load(EXPECTED + '/plan.json')
    native_plan = store.load(NATIVE + '/protocol.json')
    getter_plan = store.load(GETTERS + '/protocol.json')
    assert plan['cases'] == native_plan['cases'] == getter_plan['cases']
    assert native_plan['workdir'] == NATIVE_WORK and getter_plan['workdir'] == GETTER_WORK
    assert native_plan['limits']['processes'] == 4 and getter_plan['limits']['processes'] == 8
    assert len(getter_plan['commands']) == 8
    assert native_plan['limits']['native_bytes'] == 256 * 1024 * 1024
    assert (native_plan['limits']['native_rows'], native_plan['limits']['native_steps']) == (100000, 100000000)
    assert (native_plan['limits']['request_bytes'], native_plan['limits']['request_rows'], native_plan['limits']['value_bytes']) == (16 * 1024 * 1024, 4096, 1024)
    binding = store.load(NATIVE + '/binding-first.json')
    read_binding = store.load(GETTERS + '/binding-first.json')
    assert binding['protocol_freeze'] == store.pin(NATIVE + '/protocol-freeze.json')
    assert read_binding['controller_freeze'] == store.pin(GETTERS + '/controller-freeze.json')
    assert read_binding['producer_report'] == store.pin(NATIVE_WORK + '/report.json')
    assert read_binding['producer_binding'] == store.pin(NATIVE + '/binding-first.json')
    assert binding['workdir'] == NATIVE_WORK and read_binding['workdir'] == GETTER_WORK
    for name in ['artifact', 'build_receipt', 'source_archive']:
        pins.check(binding[name])
    receipt = store.load(binding['build_receipt']['path'])
    assert receipt['status'] == 0 and receipt['commit'] == binding['revision']
    assert receipt['artifact'] == binding['artifact'] and receipt['source_archive'] == binding['source_archive']
    assert receipt['expected_images_frozen_before_build'] == {'path': EXPECTED + '/freeze.json', 'sha256': FIXED[EXPECTED + '/freeze.json']}
    assert binding['revision'] == '876a888924ee8e05f0c2e763db58695665c669a4'
    assert binding['receipt_revision'] == 'fa0aff0ef911eb1518cee2a0248710612ae3d8d7'
    sources = []
    # Every BEFORE gate finishes before the first independent expected image.
    for case in plan['cases']:
        before = reference.inspect(case, pins.check(case['source']), pins.check(case['before_getter']), pins.check(case['before_maps']))
        sources.append(before)
    modeled = []
    for case, before in zip(plan['cases'], sources, strict=True):
        requests = reference.requests(case['variant'])
        wanted_tsv = ''.join('int32\t' + str(identity) + '\t' + ('null\t' if group is None else 'int32\t' + str(group)) + '\tbinary\t' + binary.hex() + '\n' for identity, group, binary in requests)
        assert pins.check(case['request']).read_text() == wanted_tsv
        expected, details = model.derive(case, before)
        assert pins.check(case['expected']).read_bytes() == expected
        assert store.load(pins.check(case['facts'])) == details
        modeled.append((expected, details))
    # Independent public-writer lineage remains separately replayed; none of its
    # AFTER bytes supply the engineering image constructor above.
    reference_result = verify.verify(root)
    native = command_report(store, pins, NATIVE_WORK + '/report.json', 4, 8 * 1024 * 1024)
    getters = command_report(store, pins, GETTER_WORK + '/report.json', 8, 16 * 1024 * 1024)
    assert len(native['cases']) == len(getters['cases']) == len(read_binding['candidates']) == 4
    for item in [binding['artifact'], binding['build_receipt'], binding['source_archive'], store.pin(NATIVE + '/binding-first.json')]:
        assert item in native['before']
    # All four whole images pass before interpreting even the first fresh getter.
    for index, (case, result, candidate, (expected, _)) in enumerate(zip(plan['cases'], native['cases'], read_binding['candidates'], modeled, strict=True)):
        assert case['id'] == result['id'] == candidate['id'] and result['matched']
        assert result['candidate'] == candidate['image']
        image = pins.check(candidate['image'])
        assert image == store.path(NATIVE_WORK + '/' + case['id'] + '.accdb')
        assert image.read_bytes() == expected
        expected_argv = [str(store.path(binding['artifact']['path'])), str(store.path(case['source']['path'])), str(image), 'Items', str(store.path(case['request']['path']))]
        call = native['commands'][index]
        assert call['argv'] == binding['commands'][index] == expected_argv and call['label'] == case['id']
        assert pins.check(call['stdout']).read_bytes() == f"candidate_changed_rows={case['changed_rows']}\n".encode()
        assert pins.check(call['stderr']).read_bytes() == NOTICE
    cases = []
    for index, (case, before, (_, details), outcome) in enumerate(zip(plan['cases'], sources, modeled, getters['cases'], strict=True)):
        assert outcome['id'] == case['id'] and outcome['matched'] and outcome['whole_image_before_getters']
        streams = []
        for call, template, declared in zip(getters['commands'][index * 2:index * 2 + 2], getter_plan['commands'][index * 2:index * 2 + 2], read_binding['commands'][index * 2:index * 2 + 2], strict=True):
            expected_argv = [str(store.path(read_binding['candidates'][index]['image']['path'])) if token == 'CANDIDATE' else token for token in template['argv']]
            assert call['argv'] == declared['argv'] == expected_argv
            assert call['label'] == declared['label'] == template['label']
            assert call['argv'][:3] == ['java', '-Xmx512m', '-Dstdout.encoding=UTF-8']
            assert pins.check(call['stderr']).read_bytes() == b''
            streams.append(pins.check(call['stdout']))
        after = reference.inspect(case, store.path(outcome['candidate']['path']), streams[0], streams[1], True)
        transition = reference.transition(case, before, after)
        assert transition == outcome['transition'] and reference.describe(after) == outcome['facts']
        for row in details['placements']:
            actual = after['table']['rows'][row['id']]
            assert (actual['page'], actual['slot']) == (row['page'], row['slot'])
            assert after['table']['raw_rows'][row['id']] == row['record_hex']
        cases.append({'id': case['id'], 'source_rows': before['table']['count'], 'rows': after['table']['count'],
                      'candidate': read_binding['candidates'][index]['image'], 'transition': transition})
    git = git_archive(store, pins, binding) if verify_git else None
    pins.terminal()
    return {'available': True, 'verified': True, 'candidate_images': 4, 'fresh_getter_calls_replayed': 8,
            'public_reference_cases_replayed': len(reference_result['cases']),
            'public_reference_lineage': reference_result['historical_calls'],
            'cases': cases, 'pin_occurrences': pins.occurrences, 'distinct_paths': len(pins.items),
            'separate_reference_pin_occurrences': reference_result['pin_occurrences'],
            'old_rows_preserved': sum(case['transition']['source_rows_preserved'] for case in cases),
            'inserted_rows': sum(case['transition']['inserted_rows'] for case in cases),
            'final_physical_key_ROW_pairs': 2 * sum(case['rows'] for case in cases),
            'fixture_processes': 0, 'child_processes': int(verify_git), 'git_verification': git,
            'qualification': 'Separate engineering candidates with saved independent readbacks; no native Access save/reopen qualification.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--verify-git', action='store_true', help='Explicitly run one Git metadata batch; never a fixture process')
    parser.add_argument('--evidence-root', type=Path, default=ROOT)
    args = parser.parse_args()
    try:
        result = replay(args.evidence_root, args.verify_git)
    except FileNotFoundError as error:
        print(json.dumps({'available': False, 'verified': False, 'missing': str(error), 'fixture_processes': 0, 'child_processes': 0}, indent=2))
        raise SystemExit(2)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
