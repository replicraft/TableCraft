"""Saved fitted height-one INSERT proof; optional Git is the only child process."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'docs/testing'))
from scripts.compat_artifacts import ArtifactStore
from ace16_insert_height_one import model as independent_model, reference

STORE = ArtifactStore(ROOT)
MODEL = '.cache/ace16-insert-existing-leaf-independent-next-20261027'
REFERENCE = '.cache/ace16-insert-existing-leaf-reference-next-20261027'
REFERENCE_WORK = '.cache/ace16-insert-existing-leaf-reference-first-20261027'
PRODUCER = '.cache/ace16-insert-existing-leaf-candidates-next-20261027'
GETTERS = '.cache/ace16-insert-existing-leaf-getters-next-20261027'
FIXED = {
    MODEL + '/freeze.json': 'aeb503fd16eb6c408124fd8902dda17c4952d418ec238d8741de5dc8fbf946b1',
    REFERENCE_WORK + '/report.json': 'f4aa586c468621391b67f71bda320dbbf88283264e08b46783d6f8300f0c07e6',
    PRODUCER + '/protocol-freeze.json': '72d3de9a0a4a9e996685a6d32ac18d0c95738b883c26b4b4c599b200cdd0385f',
    GETTERS + '/freeze.json': 'e024becafabb4e1b66d1d6011c4dd14320606a02788339fa135283eaa769bb57',
    GETTERS + '/controller-freeze.json': 'a0d238c5eadb8dd0c472041bb29a17d38156c39ca5ee602070b0ab46297d650f',
}
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'


class Pins:
    def __init__(self):
        self.count = 0
        self.items = {}

    def check(self, item):
        self.count += 1
        assert self.items.setdefault(item['path'], item) == item
        assert STORE.pin(item['path']) == item, item['path']
        return STORE.path(item['path'])

    def fixed(self, file, digest):
        item = STORE.pin(file)
        assert item['sha256'] == digest, file
        return self.check(item)

    def freeze(self, file):
        value = STORE.load(file)
        assert value['inputs']
        for item in value['inputs']:
            self.check(item)
        return value

    def terminal(self):
        for item in list(self.items.values()):
            self.check(item)


def portable_missing(filename):
    try:
        return Path(filename).resolve().relative_to(ROOT).as_posix()
    except (TypeError, ValueError):
        return 'required private evidence'


def maintained_sources(pins):
    original_model = STORE.path(MODEL + '/model.py').read_text()
    assert hashlib.sha256(original_model.encode()).hexdigest() == (
        '8c3179ca33d1f45daa11ca071572cd2babf8c166929dcd05821ac751567787a2')
    public_model = pins.check(STORE.pin('docs/testing/ace16_insert_height_one/model.py')).read_text()
    assert public_model == original_model.replace(
        'Path(__file__).resolve().parents[2]', 'Path(__file__).resolve().parents[3]')
    original = STORE.path(REFERENCE + '/proof.py').read_text()
    expected = original.replace('import importlib.util\n', '')
    expected = expected.replace('from pathlib import Path\n',
                                'from pathlib import Path\nfrom ace16_insert_root_split import reference as source_reference\n')
    expected = expected.replace("PREVIOUS = ROOT / '.cache/ace16-insert-root-split-reference-next-20261027'\n", '')
    begin = expected.index('def previous_proof():')
    end = expected.index('def before(case):', begin)
    expected = expected[:begin] + expected[end:]
    expected = expected.replace('parser = previous_proof()', 'parser = source_reference')
    expected = expected.replace('Path(__file__).resolve().parents[2]',
                                'Path(__file__).resolve().parents[3]')
    public = pins.check(STORE.pin('docs/testing/ace16_insert_height_one/reference.py')).read_text()
    assert public == expected
    # The imported owner is itself the maintained exact prior independent proof.
    old_private = STORE.path('.cache/ace16-insert-root-split-reference-next-20261027/proof.py').read_text()
    owner = pins.check(STORE.pin('docs/testing/ace16_insert_root_split/reference.py')).read_text()
    assert owner == old_private.replace('Path(__file__).resolve().parents[2]',
                                         'Path(__file__).resolve().parents[3]')


def commands(pins, file, count, cap):
    value = STORE.load(file)
    assert value['completed'] is True and value.get('failure') is None
    assert value['after_pin_errors'] == [] and len(value['commands']) == count
    for item in value['before'] + value['artifacts']:
        pins.check(item)
    for item in value['commands']:
        assert item['status'] == 0 and item['stopped_for'] is None
        assert item['capture_errors'] == [] and item['timeout_seconds'] == 30
        assert item['stream_limit_bytes'] == cap
        assert item['observed_stream_bytes'] == [item['stdout']['bytes'], item['stderr']['bytes']]
        assert max(item['observed_stream_bytes']) <= cap
        pins.check(item['stdout'])
        pins.check(item['stderr'])
    return value


def historical_path(token, old_root, expected):
    assert Path(token).is_absolute()
    assert Path(token).relative_to(old_root).as_posix() == expected


def git_objects(binding):
    pins = [binding['build_receipt']] + binding['compiled_sources']
    queries = [binding['receipt_revision'] + ':' + pins[0]['path']]
    queries += [binding['revision'] + ':' + item['path'] for item in pins[1:]]
    assert all('\n' not in item and '\r' not in item for item in queries)
    assert sum(item['bytes'] for item in pins) <= 64 * 1024 * 1024
    result = subprocess.run(['git', '--no-replace-objects', 'cat-file', '--batch'], cwd=ROOT,
                            input=('\n'.join(queries) + '\n').encode(),
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=True)
    position = 0
    for item in pins:
        stop = result.stdout.index(b'\n', position)
        header = result.stdout[position:stop].split()
        assert len(header) == 3 and header[1] == b'blob'
        size = int(header[2])
        begin = stop + 1
        data = result.stdout[begin:begin + size]
        assert size == item['bytes'] and hashlib.sha256(data).hexdigest() == item['sha256']
        assert result.stdout[begin + size:begin + size + 1] == b'\n'
        position = begin + size + 1
    assert position == len(result.stdout)
    return {'verified': True, 'compiled_sources': len(binding['compiled_sources']),
            'receipt_objects': 1, 'metadata_processes': 1}


def expected(pins):
    for file, digest in FIXED.items():
        pins.fixed(file, digest)
    for file in [MODEL + '/freeze.json', PRODUCER + '/protocol-freeze.json',
                 GETTERS + '/freeze.json', GETTERS + '/controller-freeze.json']:
        pins.freeze(file)
    maintained_sources(pins)
    plan = STORE.load(MODEL + '/plan.json')
    protocol = STORE.load(REFERENCE + '/plan.json')
    derived = []
    for case, selector in zip(plan['cases'], protocol['cases'], strict=True):
        assert case['id'] == selector['id'] and case['source'] == selector['source']
        assert case['request'] == selector['request']
        state = reference.before(selector)
        image, facts = independent_model.derive(state, case['request']['ID'], bytes.fromhex(case['request']['binary_hex']))
        assert facts == case['expectation']
        assert image == pins.check(case['expected_image']).read_bytes()
        assert pins.check(case['request_tsv']).read_bytes() == pins.check(selector['request']['tsv']).read_bytes()
        derived.append((case, selector, state, image, facts))
    assert len(derived) == 4
    reference_report = commands(pins, REFERENCE_WORK + '/report.json', 13, 16 * 1024 * 1024)
    assert all(a['label'] == b['label'] and a['argv'] == b['argv']
               for a, b in zip(reference_report['commands'], protocol['commands'], strict=True))
    assert reference_report['source_gates']['verified'] is True
    assert len(reference_report['source_gates']['cases']) == 4
    for case, selector, state, image, facts in derived:
        result = reference_report['references'][case['id']]
        assert result['verified'] and pins.check(result['image']).read_bytes() == image
        raw = reference.transition(selector, state, pins.check(result['image']),
                               STORE.path(REFERENCE_WORK + '/' + case['id'] + '-after-getter.stdout'),
                               STORE.path(REFERENCE_WORK + '/' + case['id'] + '-after-maps.stdout'))
        assert raw == STORE.load(result['proof']['path'])
        assert raw['new_ROW'] == facts['new_ROW']
    return plan, reference, derived


def verify(args):
    pins = Pins()
    plan, proof, derived = expected(pins)
    if args.preflight_only:
        pins.terminal()
        return {'preflight_verified': True, 'scope': 'Independent expected-image preflight only; actual verification not requested',
                'expected_images': 4, 'prior_reference_commands_verified': 13,
                'actual_candidate_acceptance': False, 'pin_occurrences': pins.count,
                'distinct_paths': len(pins.items), 'new_native_JVM_Cargo_calls': 0}
    assert args.producer_report_sha256 and args.getter_report_sha256
    pins.fixed(args.producer_report, args.producer_report_sha256)
    pins.fixed(args.getter_report, args.getter_report_sha256)
    native = commands(pins, args.producer_report, 4, 8 * 1024 * 1024)
    getter = commands(pins, args.getter_report, 8, 16 * 1024 * 1024)
    binding_pin = STORE.pin(args.producer_binding)
    assert binding_pin in native['before']
    binding = STORE.load(pins.check(binding_pin).relative_to(ROOT))
    assert binding['revision'] == '96a5e91e86948923cf382dd7c682c821aee71102'
    assert binding['receipt_revision'] == '647cd9cbc226711a818efc7dbf1f1b607669042c'
    assert binding['artifact']['sha256'] == 'aefb11f8522d0ff83ea68c4118c1157fd482e33ba04ff321f5a035fdc14ba278'
    pins.check(binding['artifact'])
    receipt = STORE.load(pins.check(binding['build_receipt']).relative_to(ROOT))
    assert receipt['exit_code'] == 0 and receipt['consumer'] == 'preview_ace16_insert'
    assert receipt['compiled_revision'] == binding['revision']
    assert receipt['frozen_example'] == binding['artifact']
    assert receipt['source_pins'] == binding['compiled_sources']
    assert len(binding['compiled_sources']) == 751
    pins.check(receipt['build_log'])
    pins.check(receipt['dependency_info'])
    assert binding['protocol_freeze'] == STORE.pin(PRODUCER + '/protocol-freeze.json')
    for revision in [binding['revision'], binding['receipt_revision']]:
        assert len(revision) == 40 and all(c in '0123456789abcdef' for c in revision)
    gb_pin = STORE.pin(args.getter_binding)
    assert gb_pin in getter['before']
    gb = STORE.load(pins.check(gb_pin).relative_to(ROOT))
    for key, file in [('getter_protocol_freeze', GETTERS + '/freeze.json'),
                      ('controller_freeze', GETTERS + '/controller-freeze.json'),
                      ('producer_protocol_freeze', PRODUCER + '/protocol-freeze.json'),
                      ('producer_report', args.producer_report)]:
        assert gb[key] == STORE.pin(file)
    assert gb['producer_report'] in getter['before']
    producer_protocol = STORE.load(PRODUCER + '/protocol.json')
    getter_protocol = STORE.load(GETTERS + '/protocol.json')
    old_root = Path(native['commands'][0]['argv'][0]).parents[len(Path(binding['artifact']['path']).parts) - 1]
    results = []
    for ordinal, (case, selector, state, image, facts) in enumerate(derived):
        actual = native['cases'][ordinal]
        command = native['commands'][ordinal]
        request = producer_protocol['cases'][ordinal]
        assert actual['id'] == command['label'] == request['id'] == case['id']
        assert actual['matched'] and actual.get('failure') is None
        assert request['source'] == case['source'] and request['expected_image'] == case['expected_image']
        assert pins.check(request['request']).read_bytes() == pins.check(case['request_tsv']).read_bytes()
        assert len(command['argv']) == 5 and command['argv'][3] == case['table']
        for token, file in [(command['argv'][0], binding['artifact']['path']),
                            (command['argv'][1], case['source']['path']),
                            (command['argv'][2], actual['candidate']['path']),
                            (command['argv'][4], request['request']['path'])]:
            historical_path(token, old_root, file)
        assert pins.check(command['stdout']).read_bytes() == b'candidate_changed_rows=1\n'
        assert pins.check(command['stderr']).read_bytes() == NOTICE
        assert pins.check(actual['candidate']).read_bytes() == image
        assert gb['candidates'][ordinal] == {'id': case['id'], 'image': actual['candidate']}
        assert getter['cases'][ordinal] == {'id': case['id'], 'whole_image_before_getters': True, 'matched': True}
        configured = getter_protocol['cases'][ordinal]
        assert configured['id'] == case['id'] and configured['expected_image'] == case['expected_image']
        assert configured['source'] == case['source'] and configured['inserted_ID'] == 227
        assert configured['inserted_binary_hex'] == case['request']['binary_hex']
        assert (configured['new_page'], configured['new_slot']) == (facts['new_page'], facts['new_slot'])
        streams = []
        for offset, label in enumerate(['getter', 'maps']):
            call = getter['commands'][ordinal * 2 + offset]
            assert call['label'] == case['id'] + '-' + label
            target = str(old_root / actual['candidate']['path'])
            assert call['argv'] == [target if token == 'CANDIDATE' else token
                                    for token in configured['commands'][offset]]
            assert call['argv'][:3] == ['java', '-Xmx512m', '-Dstdout.encoding=UTF-8']
            assert pins.check(call['stderr']).read_bytes() == b''
            stream = pins.check(call['stdout'])
            cached = configured['expected_getter' if offset == 0 else 'expected_maps']
            assert stream.read_bytes() == pins.check(cached).read_bytes()
            streams.append(stream)
        result = proof.transition(selector, state, pins.check(actual['candidate']), *streams)
        assert result['new_ROW'] == facts['new_ROW']
        assert result['public_unused_tail_differences'] == []
        results.append({'id': case['id'], 'candidate': actual['candidate'], 'new_ROW': facts['new_ROW'],
                        'whole_image': True, 'old_ROWS_preserved': 452, 'final_rows': 453,
                        'new_pages': 0, 'maps_unchanged': True})
    git = git_objects(binding) if args.verify_git else {'verified': False, 'metadata_processes': 0}
    pins.terminal()
    return {'verified': True, 'available': True, 'scope': 'Four saved engineering candidates only; no native Access save claim',
            'producer_commands': 4, 'fresh_getter_commands': 8, 'prior_reference_commands': 13,
            'complete_key_ROW_entries': 1812, 'old_ROWS_preserved': 1808, 'cases': results,
            'pin_occurrences': pins.count, 'distinct_paths': len(pins.items),
            'Git': git, 'new_native_JVM_Cargo_calls': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preflight-only', action='store_true')
    parser.add_argument('--verify-git', action='store_true')
    parser.add_argument('--producer-report', default='.cache/ace16-insert-existing-leaf-candidates-first-20261027/report.json')
    parser.add_argument('--producer-report-sha256', default='e11a3a4364aa5f1e083c26a33eae9debec6dc97fc7ee4abe784e452812b4d6c7')
    parser.add_argument('--getter-report', default='.cache/ace16-insert-existing-leaf-getters-first-20261027/report.json')
    parser.add_argument('--getter-report-sha256', default='7159e35befe21c79beeadefb1cadc6f286595ff88218ba699912975dcb52f893')
    parser.add_argument('--producer-binding', default=PRODUCER + '/binding-first.json')
    parser.add_argument('--getter-binding', default=GETTERS + '/binding-first.json')
    args = parser.parse_args()
    try:
        if not args.preflight_only:
            STORE.path(args.producer_report).stat()
            STORE.path(args.getter_report).stat()
        result = verify(args)
    except FileNotFoundError as error:
        print(json.dumps({'verified': False, 'available': False,
                          'reason': 'Required private evidence is absent',
                          'missing': portable_missing(error.filename)}, indent=2))
        return 2
    except (AssertionError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(json.dumps({'verified': False, 'available': True,
                          'reason': type(error).__name__ + ': ' + str(error)}, indent=2))
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
