"""Independent saved byte/receipt check; no producer or proof code is imported."""
import argparse
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from scripts.compat_artifacts import ArtifactStore

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
NOTICE = b'Engineering candidate: delete actions, data-macro semantics and native Access qualification pending.\n'


def verify():
    store = ArtifactStore(ROOT)
    pins = 0

    def check(item):
        nonlocal pins
        assert store.pin(item['path']) == item, item['path']
        pins += 1
        return store.path(item['path'])

    freeze = store.load(HERE / 'freeze.json')
    for item in freeze['inputs']:
        check(item)
    plan = store.load(HERE / 'plan.json')
    binding = store.load(HERE / 'getter-binding.json')
    for item in binding['inputs']:
        check(item)
    check(plan['artifact'])
    check(plan['receipt'])
    receipt = store.load(plan['receipt']['path'])
    assert receipt['commit'] == '50d028cc19452c54046b1e1d23baa77ac80f491b'
    assert receipt['status'] == 0 and receipt['artifacts']['batch'] == plan['artifact']
    native = store.load(Path(plan['native_workdir']) / 'report.json')
    getters = store.load(Path(plan['getter_workdir']) / 'report.json')
    assert native['completed'] and native['phase'] == 'native' and native['pin_errors'] == []
    assert getters['completed'] and getters['phase'] == 'getters' and getters['pin_errors'] == []
    assert len(plan['cases']) == len(native['commands']) == len(native['whole_images']) == 20
    assert len(getters['commands']) == len(getters['proofs']) == 20
    total = 0
    pairs = []
    for case, command, image, reader, proof in zip(
            plan['cases'], native['commands'], native['whole_images'],
            getters['commands'], getters['proofs'], strict=True):
        check(case['source'])
        check(case['before_getter'])
        expected = check(case['expected']).read_bytes()
        observed_pin = store.pin(case['output'])
        observed = check(observed_pin).read_bytes()
        assert observed == expected
        assert image == {'id': case['id'], 'image': observed_pin}
        for actual, planned in [(command, case['native']), (reader, case['getter'])]:
            assert actual['argv'] == planned['argv']
            assert actual['label'] == planned['label']
            assert actual['status'] == 0 and actual['stopped_for'] is None
            assert actual['capture_errors'] == []
            assert actual['timeout_seconds'] == 30 and actual['stream_limit_bytes'] == 16 * 1024 * 1024
            assert actual['observed_stream_bytes'] == [actual['stdout']['bytes'], actual['stderr']['bytes']]
        assert check(command['stdout']).read_bytes() == f"candidate_changed_rows={case['changed_rows']}\n".encode()
        assert check(command['stderr']).read_bytes() == NOTICE
        check(reader['stdout'])
        assert check(reader['stderr']).read_bytes() == b''
        assert proof['id'] == case['id'] and proof['facts']['id'] == case['id']
        assert proof['facts']['output_sha256'] == observed_pin['sha256']
        assert proof['facts']['source_sha256'] == case['source']['sha256']
        total += len(observed)
        pairs.append({'id': case['id'], 'bytes': len(observed),
                      'sha256': observed_pin['sha256'], 'exact': True})
    assert total == 9_928_704
    return {'verified': True, 'source_files': receipt['exported_file_count'],
            'source_originals': len({case['source']['path'] for case in plan['cases']}),
            'native_commands': 20, 'fresh_readers': 20, 'exact_images': 20,
            'total_image_bytes': total, 'pairs': pairs, 'pins': pins,
            'children': 0, 'absence_or_native_Access_qualified': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = verify()
    text = json.dumps(result, indent=2) + '\n'
    if args.output is not None:
        with args.output.open('x') as output:
            output.write(text)
    print(json.dumps({key: value for key, value in result.items() if key != 'pairs'}))


if __name__ == '__main__':
    main()
