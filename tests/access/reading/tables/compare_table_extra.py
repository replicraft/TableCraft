"""Explicit bounded inert table-Extra comparisons against a caller's private manifest.

No fixture bodies are embedded here. This launches only the pinned example for
the finite manifest inventory; it never launches Access, Java or stored content.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import threading
import time

ROOT = Path(__file__).resolve().parents[2]


def pin(path):
    path = Path(path)
    raw = path.read_bytes()
    return {'path': str(path.resolve().relative_to(ROOT)), 'bytes': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest()}


def require(expected):
    assert pin(ROOT / expected['path']) == expected, expected['path']


def group_bodies(raw):
    assert raw[:4] == b'MR2\0'
    position = 4
    result = []
    while position < len(raw):
        length = int.from_bytes(raw[position:position + 4], 'little')
        kind = int.from_bytes(raw[position + 4:position + 6], 'little')
        assert length >= 6 and position + length <= len(raw)
        if kind != 128:
            result.append((kind, raw[position + 6:position + length].hex()))
        position += length
    assert position == len(raw)
    return result


def compare(stdout, case, golden):
    lines = [line.split('\t') for line in stdout.decode('ascii', errors='strict').splitlines()]
    origin = case['origin']
    expected = [
        ['TABLE_EXTRA', '1'],
        ['OBJECT', str(origin['Id']), str(origin['ParentId']), str(origin['page']),
         str(origin['slot']), str(origin['Type']), str(origin['Flags']),
         case['table'].encode('utf-8').hex()],
    ]
    raw = (ROOT / golden['extra']['path']).read_bytes()
    expected.append(['PAYLOAD', 'Present', 'WideMr2', raw.hex()])
    properties = golden['expected_properties']
    for index, name in enumerate(properties['names']):
        expected.append(['NAME', str(index), name.encode('utf-8').hex()])
    bodies = group_bodies(raw)
    assert len(bodies) == len(properties['groups'])
    entries = 0
    for index, (group, (kind, body)) in enumerate(zip(properties['groups'], bodies)):
        assert group['kind'] == kind
        expected.append(['GROUP', str(index), str(kind), 'true', body])
        for position, value in enumerate(group['properties']):
            entries += 1
            expected.append(['PROPERTY', str(index), str(position), str(value['flags']),
                             str(value['native_type']), value['name'].encode('utf-8').hex(),
                             value['raw_hex'], value['text'].encode('utf-8').hex()])
    assert entries == properties['entries']
    assert len(lines) == len(expected), ('line count', len(lines), len(expected))
    for index, (actual, wanted) in enumerate(zip(lines, expected)):
        assert actual == wanted, ('mismatch at record', index, wanted[0])
    return {'raw_bytes': len(raw), 'names': len(properties['names']),
            'groups': len(bodies), 'properties': entries, 'exact_records': len(expected)}


def capture(argv, case, work):
    caps = [case['stdout_cap'], case['stderr_cap']]
    timeout = case['timeout_seconds']
    assert all(0 < cap <= 16 * 1024 * 1024 for cap in caps)
    assert 0 < timeout <= 60
    observed = [0, 0]
    failures = []
    stopped = threading.Event()
    paths = [work / f"{case['id']}.stdout", work / f"{case['id']}.stderr"]

    def drain(source, path, index):
        try:
            with path.open('xb') as output:
                while chunk := source.read(65536):
                    output.write(chunk[:max(0, caps[index] - observed[index])])
                    observed[index] += len(chunk)
                    if observed[index] > caps[index]:
                        stopped.set()
        except BaseException as error:
            failures.append(type(error).__name__)
            stopped.set()
        finally:
            source.close()

    child = subprocess.Popen(argv, cwd=ROOT, stdin=subprocess.DEVNULL,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    threads = []
    reason = None
    try:
        for index, source in enumerate([child.stdout, child.stderr]):
            thread = threading.Thread(target=drain, args=(source, paths[index], index), daemon=True)
            thread.start()
            threads.append(thread)
        deadline = time.monotonic() + timeout
        while child.poll() is None:
            if stopped.is_set() or time.monotonic() >= deadline:
                reason = 'capture limit/error' if stopped.is_set() else 'timeout'
                child.kill()
                break
            time.sleep(0.02)
        status = child.wait(timeout=2)
    finally:
        if child.poll() is None:
            child.kill()
            child.wait(timeout=2)
        for thread in threads:
            thread.join(timeout=2)
    if any(thread.is_alive() for thread in threads):
        reason = 'pipe did not close'
    if stopped.is_set():
        reason = 'capture limit/error'
    return {'argv': argv, 'status': status, 'stopped_for': reason, 'errors': failures,
            'timeout_seconds': timeout, 'stream_caps': caps, 'observed_bytes': observed,
            'stdout': pin(paths[0]), 'stderr': pin(paths[1])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--example', required=True, type=Path)
    parser.add_argument('--manifest', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--static-only', action='store_true')
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text())
    require(manifest['example'])
    assert args.example.resolve() == (ROOT / manifest['example']['path']).resolve()
    assert len(manifest['calls']) == manifest['command_count']
    assert len({case['id'] for case in manifest['calls']}) == manifest['command_count']
    for item in manifest['inputs']:
        require(item)
    goldens = json.loads((ROOT / manifest['goldens']['path']).read_text())
    require(manifest['goldens'])
    assert len({case['golden_index'] for case in manifest['calls']}) == len(goldens['items'])
    for case in manifest['calls']:
        golden = goldens['items'][case['golden_index']]
        require(golden['extra'])
        assert case['origin']['Id'] == golden['object_native_id']
        assert case['origin']['ParentId'] == golden['parent_native_id']
        assert case['origin']['Type'] == golden['type_code'] == 1
        assert case['table'] == json.loads(golden['name_display'])
    if args.static_only:
        print(json.dumps({'static': True, 'commands': manifest['command_count'], 'children': 0}))
        return
    args.output.mkdir(exist_ok=False)
    report = {'completed': False, 'manifest': pin(args.manifest), 'commands': [], 'comparisons': []}
    try:
        for case in manifest['calls']:
            require(report['manifest'])
            for item in manifest['inputs']:
                require(item)
            require(manifest['example'])
            argv = [str(args.example.resolve()), case['image'], case['table'], *case['arguments']]
            record = capture(argv, case, args.output)
            report['commands'].append(record)
            assert record['status'] == 0 and record['stopped_for'] is None and not record['errors']
            assert record['stderr']['bytes'] == 0
            stdout = (ROOT / record['stdout']['path']).read_bytes()
            proof = compare(stdout, case, goldens['items'][case['golden_index']])
            report['comparisons'].append(dict(id=case['id'], **proof))
            for item in manifest['inputs']:
                require(item)
        report['completed'] = True
        require(manifest['example'])
    except BaseException as error:
        report['failure'] = {'type': type(error).__name__, 'message': str(error)}
        raise
    finally:
        report['after_pin_errors'] = []
        for item in manifest['inputs']:
            try:
                require(item)
            except BaseException:
                report['after_pin_errors'].append(item['path'])
        (args.output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    assert not report['after_pin_errors']
    print(json.dumps({'completed': True, 'commands': len(report['commands']),
                      'properties': sum(item['properties'] for item in report['comparisons'])}))


if __name__ == '__main__':
    main()
