"""Reprove four saved INSERT images; only optional batched Git reads spawn."""
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
from ace16_insert_candidates import model
from slot_preview_originals import facts

STORE = ArtifactStore(ROOT)
ORACLE = '.cache/ace16-insert-independent-next-20261027'
PRODUCER = '.cache/ace16-insert-candidates-next-20261027'
GETTERS = '.cache/ace16-insert-getters-next-20261027'
MODEL_SHA = 'e25461040e1315054d2ce30771481262e0f59fa9b703b9cf5a8c294a942680be'
COMPILED = '4f9ee2da2f38651ea145209e8d568f26bc15a58d'
RECEIPT_REVISION = 'c399fce61fce657a33f8cb86acc6f6a962a9f54c'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
DEPENDENCY_ARCHIVE = PRODUCER + '/committed-dependency-info.d'
DEPENDENCY_SHA = '5b1c9de93592c1850972b99f68b81aac83cbe5a3e6005f31dbe76bf418a6dd17'
FIXED = {
    ORACLE + '/freeze.json': 'ba83843f9705046c95e032f10b0f681f30a0a7a2e7d0ee6c67055cbe5555d7d7',
    ORACLE + '/plan.json': 'ff97ab60f64c8289b8fad4576aa25107b4cd085fb7bbbf4def03a0c76c502ffe',
    PRODUCER + '/protocol-freeze.json': '9a5f92371a7e3dc7790a026d1930dee1fa24045fb37e6110647e9d0a1b09a649',
    PRODUCER + '/binding-first.json': 'a91b283b036a7a740cf4be37896b871f8d6c731177c5069b16c7263ebe8b8c98',
    PRODUCER + '/first-actual/report.json': '88711524eeac10a8062b1d6e9971d64bfdecf9faf1b4b1cb3db1e0334e2b211a',
    GETTERS + '/freeze.json': '07b8954f9e24613ac9ca67dcf439aaf755e58739c231e6a969fd9c21f9c24649',
    GETTERS + '/controller-freeze.json': '601da516311e80bf9c1094ce44f917637be090b75d74112a7b2a50d22e97fa08',
    GETTERS + '/binding-first.json': '196a815feabadf4646947d7fec758c9eb11250edec9bedfdd8e57847d0328f1b',
    GETTERS + '/first-actual/report.json': '734e8e24b8571212ed1442f898c13f242d5ca7432020e972ed90ad70b9f02cc0',
}
CASE_IDS = ['V2016-fitted', 'V2016-EOF', 'V2019-fitted', 'V2019-EOF']


class Unavailable(Exception):
    """A required private artifact or requested historical Git object is absent."""


class Pins:
    """Check every pin occurrence afresh while detecting contradictory identities."""

    def __init__(self):
        self.occurrences = 0
        self.identities = {}

    def check(self, expected):
        self.occurrences += 1
        prior = self.identities.setdefault(expected['path'], expected)
        assert prior == expected, 'Contradictory artifact identity: ' + expected['path']
        assert STORE.pin(expected['path']) == expected, expected['path']
        return STORE.path(expected['path'])

    def fixed(self, path, sha256):
        actual = STORE.pin(path)
        assert actual['sha256'] == sha256, path
        self.check(actual)


def git_objects(binding, receipt):
    """Read the receipt and complete compiled source closure in one Git process."""
    expected = [binding['build_receipt']] + receipt['source_pins']
    queries = [RECEIPT_REVISION + ':' + expected[0]['path']]
    queries.extend(COMPILED + ':' + item['path'] for item in expected[1:])
    assert len(queries) == 746
    assert all('\n' not in query and '\r' not in query for query in queries)
    assert sum(item['bytes'] for item in expected) < 16 * 1024 * 1024
    result = subprocess.run(
        ['git', '--no-replace-objects', 'cat-file', '--batch'],
        cwd=ROOT,
        input=('\n'.join(queries) + '\n').encode(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=30,
        check=True,
    )
    position = 0
    for query, item in zip(queries, expected, strict=True):
        stop = result.stdout.index(b'\n', position)
        header = result.stdout[position:stop].split()
        if len(header) == 2 and header[1] == b'missing':
            raise Unavailable('Required historical Git object is unavailable: ' + query)
        assert len(header) == 3 and header[1] == b'blob', query
        size = int(header[2])
        begin = stop + 1
        end = begin + size
        blob = result.stdout[begin:end]
        assert size == item['bytes'], query
        assert hashlib.sha256(blob).hexdigest() == item['sha256'], query
        assert result.stdout[end:end + 1] == b'\n'
        position = end + 1
    assert position == len(result.stdout)
    return {'verified': True, 'source_objects': 745, 'receipt_objects': 1, 'processes': 1}


def report(pins, path, command_count, stream_limit):
    value = STORE.load(path)
    assert value['completed'] is True and value.get('failure') is None
    assert value['after_pin_errors'] == []
    assert len(value['commands']) == command_count and len(value['cases']) == 4
    for item in value['before'] + value['artifacts']:
        pins.check(item)
    for command in value['commands']:
        assert command['status'] == 0 and command['stopped_for'] is None
        assert command['capture_errors'] == []
        assert command['timeout_seconds'] == 30
        assert command['stream_limit_bytes'] == stream_limit
        assert command['observed_stream_bytes'] == [
            command['stdout']['bytes'], command['stderr']['bytes'],
        ]
        assert all(size <= stream_limit for size in command['observed_stream_bytes'])
        pins.check(command['stdout'])
        pins.check(command['stderr'])
    return value


def frozen(pins, path):
    value = STORE.load(path)
    for item in value['inputs']:
        pins.check(item)
    return value


def read_pin(pins, item):
    return pins.check(item).read_bytes()


def historical_path(token, root, expected):
    """Check recorded absolute argv without requiring the old checkout location."""
    actual = Path(token)
    assert actual.is_absolute()
    assert actual.relative_to(root).as_posix() == expected



def dependency_archive(pins, receipt, recorded_root):
    """Check retained historical bytes without depending on a disposable target."""
    archived = STORE.pin(DEPENDENCY_ARCHIVE)
    assert archived['bytes'] == 79924 and archived['sha256'] == DEPENDENCY_SHA
    assert archived['bytes'] == receipt['dependency_info']['bytes']
    assert archived['sha256'] == receipt['dependency_info']['sha256']
    text = pins.check(archived).read_text()
    assert len(text.splitlines()) == 1
    target, paths = text.strip().split(': ', 1)
    historical_path(target, recorded_root, 'target/debug/examples/preview_ace16_insert')
    assert '\\ ' not in paths
    dependencies = {Path(path).relative_to(recorded_root).as_posix() for path in paths.split()}
    assert len(dependencies) == 733
    source_paths = {item['path'] for item in receipt['source_pins']}
    assert dependencies <= source_paths
    additional = source_paths - dependencies
    assert len(additional) == 12
    assert all(
        path in ('Cargo.lock', 'Cargo.toml', 'rust-toolchain.toml')
        or path.endswith('/Cargo.toml')
        for path in additional
    )
    return {
        'archive': archived,
        'matches_historical_receipt_bytes': True,
        'dependency_source_paths': 733,
        'additional_manifest_lock_toolchain_paths': 12,
        'historical_location': receipt['dependency_info']['path'],
        'historical_location_rechecked': False,
    }


def verify(verify_git=False):
    pins = Pins()
    for path, sha256 in FIXED.items():
        pins.fixed(path, sha256)
    pins.fixed('docs/testing/ace16_insert_candidates/model.py', MODEL_SHA)
    assert STORE.path('docs/testing/ace16_insert_candidates/model.py').read_bytes() == (
        STORE.path(ORACLE + '/model.py').read_bytes()
    )

    original = frozen(pins, ORACLE + '/freeze.json')
    assert len(original['inputs']) == 175
    assert original['plan'] == STORE.pin(ORACLE + '/plan.json')
    pins.check(original['requests'])
    plan = STORE.load(original['plan']['path'])
    assert [case['id'] for case in plan['cases']] == CASE_IDS
    producer_freeze = frozen(pins, PRODUCER + '/protocol-freeze.json')
    getter_freeze = frozen(pins, GETTERS + '/freeze.json')
    controller_freeze = frozen(pins, GETTERS + '/controller-freeze.json')
    assert producer_freeze['inputs'] and getter_freeze['inputs'] and controller_freeze['inputs']
    native = report(pins, PRODUCER + '/first-actual/report.json', 4, 8 * 1024 * 1024)
    getters = report(pins, GETTERS + '/first-actual/report.json', 8, 16 * 1024 * 1024)
    assert [case['id'] for case in native['cases']] == CASE_IDS
    assert [case['id'] for case in getters['cases']] == CASE_IDS

    binding = STORE.load(PRODUCER + '/binding-first.json')
    assert binding['revision'] == COMPILED
    assert binding['receipt_revision'] == RECEIPT_REVISION
    assert binding['protocol_freeze'] == STORE.pin(PRODUCER + '/protocol-freeze.json')
    pins.check(binding['artifact'])
    pins.check(binding['build_receipt'])
    receipt = STORE.load(binding['build_receipt']['path'])
    assert receipt['exit_code'] == 0 and receipt['consumer'] == 'preview_ace16_insert'
    assert receipt['compiled_revision'] == COMPILED
    assert receipt['frozen_example'] == binding['artifact']
    assert receipt['source_pins'] == binding['compiled_sources']
    assert len(receipt['source_pins']) == 745
    pins.check(receipt['build_log'])

    getter_binding = STORE.load(GETTERS + '/binding-first.json')
    for name, path in [
        ('getter_protocol_freeze', GETTERS + '/freeze.json'),
        ('controller_freeze', GETTERS + '/controller-freeze.json'),
        ('producer_report', PRODUCER + '/first-actual/report.json'),
        ('producer_protocol_freeze', PRODUCER + '/protocol-freeze.json'),
    ]:
        assert getter_binding[name] == STORE.pin(path)
    assert getter_binding['producer_report'] in getters['before']
    producer_protocol = STORE.load(PRODUCER + '/protocol.json')
    getter_protocol = STORE.load(GETTERS + '/protocol.json')
    assert [case['id'] for case in producer_protocol['cases']] == CASE_IDS
    assert [case['id'] for case in getter_protocol['cases']] == CASE_IDS
    assert len(getter_binding['candidates']) == 4

    artifact_parts = len(Path(binding['artifact']['path']).parts)
    recorded_root = Path(native['commands'][0]['argv'][0]).parents[artifact_parts - 1]
    dependency = dependency_archive(pins, receipt, recorded_root)
    cases = []
    table_count = 0
    typed_rows = 0
    raw_rows = 0
    logical_entries = 0
    physical_entries = 0
    selected_entries = 0
    old_selected_rows = 0
    map_bytes = 0
    growth = 0
    for number, case in enumerate(plan['cases']):
        identifier = case['id']
        source = read_pin(pins, case['source'])
        source_text = read_pin(pins, case['source_getter']).decode('utf-8')
        source_tables = facts.parse(source_text)
        values = case['request']['values']
        assert len(values) == 2
        assert values[0]['kind'] == 'int32' and values[1]['kind'] == 'binary'
        identity = values[0]['value']
        payload = bytes.fromhex(values[1]['hex'])
        assert len(payload) == 255
        # Derive all bytes from the immutable BEFORE and explicit request first.
        expected, details = model.derive(
            source, source_tables[case['table']], identity, payload,
        )
        assert details == case['expectation']
        assert len(expected) == case['expected_image']['bytes']
        assert model.digest(expected) == case['expected_image']['sha256']
        assert expected == read_pin(pins, case['expected_image'])
        assert expected == read_pin(pins, case['actual_public_reference'])

        native_case = native['cases'][number]
        command = native['commands'][number]
        requested = producer_protocol['cases'][number]
        assert native_case['matched'] is True and native_case.get('failure') is None
        assert command['label'] == identifier
        assert requested['source'] == case['source']
        assert requested['expected_image'] == case['expected_image']
        request = read_pin(pins, requested['request'])
        assert request == f'int32\t{identity}\tbinary\t{payload.hex()}\n'.encode()
        argv = command['argv']
        assert len(argv) == 5 and argv[3] == case['table']
        for token, path in [
            (argv[0], binding['artifact']['path']),
            (argv[1], case['source']['path']),
            (argv[2], native_case['candidate']['path']),
            (argv[4], requested['request']['path']),
        ]:
            historical_path(token, recorded_root, path)
        assert read_pin(pins, command['stdout']) == b'candidate_changed_rows=1\n'
        assert read_pin(pins, command['stderr']) == NOTICE
        assert expected == read_pin(pins, native_case['candidate'])
        assert getter_binding['candidates'][number] == {
            'id': identifier, 'image': native_case['candidate'],
        }
        # Candidate equality is established before interpreting either saved getter.
        inspection = getters['cases'][number]
        configured = getter_protocol['cases'][number]
        assert inspection == {'id': identifier, 'whole_image_before_getters': True, 'matched': True}
        assert configured['source'] == case['source']
        assert configured['source_getter'] == case['source_getter']
        assert configured['source_maps'] == case['source_maps']
        assert configured['expected_image'] == case['expected_image']
        assert configured['inserted_ID'] == identity
        assert configured['inserted_binary_hex'] == payload.hex()
        assert configured['new_page'] == details['page']
        assert configured['new_slot'] == details['slot']
        assert configured['final_count'] == details['final_count']
        outputs = []
        for offset, label in enumerate(('getter', 'maps')):
            actual = getters['commands'][2 * number + offset]
            template = configured['commands'][offset]
            candidate_path = str(recorded_root / native_case['candidate']['path'])
            expected_argv = [candidate_path if token == 'CANDIDATE' else token for token in template]
            assert actual['label'] == identifier + '-' + label
            assert actual['argv'] == expected_argv
            assert actual['argv'][0:3] == ['java', '-Xmx512m', '-Dstdout.encoding=UTF-8']
            assert read_pin(pins, actual['stderr']) == b''
            output = read_pin(pins, actual['stdout'])
            key = 'expected_getter' if offset == 0 else 'expected_maps'
            assert output == read_pin(pins, configured[key])
            outputs.append(output)

        tables = facts.parse(outputs[0].decode('utf-8'))
        assert tables == facts.parse(read_pin(pins, configured['expected_getter']).decode('utf-8'))
        selected = tables[case['table']]
        assert selected['count'] == details['final_count']
        assert selected['rows'][identity] == {
            'page': details['page'], 'slot': details['slot'],
            'values': ['Integer:' + str(identity), 'Binary:' + payload.hex()],
        }
        for old_id, old_row in source_tables[case['table']]['rows'].items():
            assert selected['rows'][old_id] == old_row
            assert selected['raw_rows'][old_id] == source_tables[case['table']]['raw_rows'][old_id]
        for table in tables.values():
            assert table['count'] == len(table['rows']) == len(table['raw_rows'])
            table_count += 1
            typed_rows += len(table['rows'])
            raw_rows += len(table['raw_rows'])
            owners = {}
            for index in table['indexes'].values():
                assert index['entries'] == len(index['order'])
                logical_entries += len(index['order'])
                prior = owners.setdefault(index['physical'], index)
                assert prior['root'] == index['root']
                assert prior['order'] == index['order']
            physical_entries += sum(len(index['order']) for index in owners.values())
        selected_entries += details['final_count']
        old_selected_rows += details['source_count']
        map_bytes += len(details['map_byte_effects'])
        growth += len(expected) - len(source)
        cases.append({
            'id': identifier,
            'candidate': native_case['candidate'],
            'placement': details['placement'],
            'reference': details['reference'],
            'source_count': details['source_count'],
            'final_count': details['final_count'],
            'source_historical_count': details['source_historical_count'],
            'final_historical_count': details['final_historical_count'],
            'record_bytes': details['record_bytes'],
            'map_byte_effects': details['map_byte_effects'],
            'rederived_whole_image_before_getter_interpretation': True,
            'recorded_whole_image_before_getter_calls': True,
            'fresh_getter_and_maps_equal_saved_public_reference': True,
        })

    git = git_objects(binding, receipt) if verify_git else {
        'verified': False, 'source_objects': 0, 'receipt_objects': 0, 'processes': 0,
    }
    verification_occurrences = pins.occurrences
    terminal_pins = list(pins.identities.values())
    for item in terminal_pins:
        pins.check(item)
    return {
        'verified': True,
        'available': True,
        'scope': 'Four saved engineering INSERT candidates; no native Access save/application qualification',
        'producer_commands': 4,
        'getter_commands': 8,
        'new_native_JVM_Cargo_calls': 0,
        'original_model_closure': 175,
        'verification_pin_occurrences': verification_occurrences,
        'terminal_rehashed_artifacts': len(terminal_pins),
        'pin_occurrences': pins.occurrences,
        'distinct_artifacts': len(pins.identities),
        'tables_observed': table_count,
        'typed_rows': typed_rows,
        'raw_rows': raw_rows,
        'logical_index_entries': logical_entries,
        'physical_index_entries': physical_entries,
        'selected_final_primary_entries': selected_entries,
        'old_selected_rows_preserved': old_selected_rows,
        'map_byte_changes': map_bytes,
        'appended_bytes': growth,
        'compiled_revision': COMPILED,
        'receipt_revision': RECEIPT_REVISION,
        'Git': git,
        'dependency_info': dependency,
        'cases': cases,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--verify-git', action='store_true', help='Read 746 historical blobs in one local Git process')
    args = parser.parse_args()
    try:
        result = verify(args.verify_git)
    except FileNotFoundError as error:
        print(json.dumps({
            'verified': False, 'available': False,
            'reason': 'Required private evidence is unavailable',
            'missing': str(error.filename),
        }, indent=2))
        return 2
    except Unavailable as error:
        print(json.dumps({'verified': False, 'available': False, 'reason': str(error)}, indent=2))
        return 2
    except (AssertionError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(json.dumps({
            'verified': False, 'available': True,
            'reason': type(error).__name__ + ': ' + str(error),
        }, indent=2))
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
