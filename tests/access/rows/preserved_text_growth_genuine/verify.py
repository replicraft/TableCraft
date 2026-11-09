"""Verify saved genuine growth evidence; never invoke a database process."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import tarfile

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'scripts'))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from compat_artifacts import ArtifactStore
from preserved_text_growth_genuine import prepare
from preserved_text_growth_genuine.model import require

ROOT = Path(__file__).resolve().parents[3]
DEFAULT_RECORD = 'docs/testing/results/ace16-preserved-text-growth-genuine-images-2026-10-30.json'
REPORT_HEADER = b'Engineering copy\tExperimental; data macros and native Access saves unqualified\n'


def verify(root, record_path):
    store = ArtifactStore(root)
    checked = set()

    def check(pin):
        expected = {key: pin[key] for key in ['path', 'bytes', 'sha256']}
        require(store.pin(pin['path']) == expected, 'Saved artifact changed: ' + pin['path'])
        checked.add((pin['path'], pin['sha256']))

    def read(pin):
        check(pin)
        data = store.path(pin['path']).read_bytes()
        require(len(data) == pin['bytes'] and hashlib.sha256(data).hexdigest() == pin['sha256'],
                'Actual read matches saved identity')
        return data

    def load(pin):
        return json.loads(read(pin).decode('utf-8'))

    def streams(call):
        require(call['started'] and call['returncode'] == 0 and call['capture_error'] is None,
                'Saved successful process outcome')
        require(len(call['logs']) == 2, 'Saved stdout/stderr pair')
        stdout, stderr = (read(pin) for pin in call['logs'])
        require(not stderr, 'Saved stderr empty')
        return stdout

    record = store.load(record_path)
    for pin in record['maintained_files']:
        check(pin)
    evidence = record['evidence']
    saved = {}
    for name, pin in evidence.items():
        if pin['path'].endswith('.json'):
            saved[name] = load(pin)
        else:
            check(pin)
    expected = saved['expectations']
    requests = saved['requests']
    report = saved['expected_report']
    require(report['completed'] and report['failure'] is None
            and report['completed_cases'] == report['expected_images'] == 8
            and report['refusal_controls'] == report['child_calls'] == report['writer_calls'] == 0,
            'Independent complete eight-image construction')
    require(expected['request_inventory'] == evidence['requests']
            and requests['original_maps'] == evidence['original_maps'], 'Exact original authority')
    for pin in expected['inputs']:
        check(pin)
    for name in ['source_plan', 'native_plan', 'sdk_plan']:
        for pin in saved[name]['inputs']:
            check(pin)

    source = saved['source_receipt']
    require(source['completed'] and source['failure'] is None
            and source['native_or_cli_producer_calls'] == 0
            and [source[key] for key in ['planned_children', 'started_children', 'completed_children']]
            == [5, 5, 5], 'Original-only five-call source qualification')
    require([call['argv'] for call in source['calls']]
            == [command['argv'] for command in saved['source_plan']['commands']], 'Exact source calls')
    for call in source['calls']:
        streams(call)
    for pin in source['retained_classes'] + source['retained_originals']:
        check(pin)
    for pin in source['inputs']:
        check(pin)
    originals = {}
    for pin in source['retained_originals']:
        raw = read(pin)
        call = next(call for call in source['calls'] if call['id'] == f'inspect-h{raw[20]}')
        require(call['argv'][-1] == pin['path'], 'Exact original getter input')
        originals[raw[20]] = raw, streams(call).decode('utf-8'), call
    require(set(originals) == {5, 6}, 'Both immutable source headers')

    export = saved['export_receipt']
    build = saved['build_receipt']
    historical_root = Path(saved['native_receipt']['calls'][0]['argv'][0])
    for _ in Path(build['cli_artifact']['path']).parts:
        historical_root = historical_root.parent
    require(export['revision'] == record['producer_revision'] and export['all_files_match_git_objects']
            and export['target_initially_absent'] and export['producer_invocations'] == 0,
            'Committed isolated producer export')
    check(export['source_archive'])
    inventory = load(export['source_inventory'])
    require(len(inventory) == export['source_files'], 'Complete archived source inventory')
    seen = set()
    with tarfile.open(store.path(export['source_archive']['path']), mode='r:') as archive:
        require(archive.pax_headers.get('comment') == record['producer_revision'],
                'Archive commit identity')
        for member in archive:
            if member.isdir():
                continue
            require(member.isfile() and member.name in inventory and member.name not in seen,
                    'Unique ordinary archived source file')
            seen.add(member.name)
            metadata = inventory[member.name]
            require(member.size == metadata['bytes']
                    and bool(member.mode & 0o111) == metadata['executable'], 'Archived source metadata')
            digest = hashlib.sha256()
            blob = hashlib.sha1(b'blob ' + str(member.size).encode('ascii') + b'\0')
            with archive.extractfile(member) as source_file:
                for chunk in iter(lambda: source_file.read(65536), b''):
                    digest.update(chunk)
                    blob.update(chunk)
            require(digest.hexdigest() == metadata['sha256']
                    and blob.hexdigest() == metadata['git_blob'], 'Actual archived source bytes/blob')
    require(seen == set(inventory), 'Exact complete archive file membership')
    require(build['completed'] and build['exit_code'] == 0 and build['producer_invocations'] == 0
            and build['source_inventory_unchanged_after_build']
            and build['owning_engine_adapter_cli_compilation_observed'], 'Qualified producer build')
    check(build['cli_artifact'])
    for pin in build['compiler_artifacts']:
        check(pin)
    stdout = []
    for pin in build['logs']:
        data = read(pin)
        if Path(pin['path']).name == 'build.stdout.jsonl':
            stdout.append(data)
    require(len(stdout) == 1, 'One pinned compiler event stream')
    events = [json.loads(line) for line in stdout[0].decode('utf-8').splitlines()]
    source_root = historical_root / Path(export['source_archive']['path']).parent / 'source'
    owners = [
        ('crates/engines/tablecraft-ace/Cargo.toml', 'tablecraft_ace', ['lib']),
        ('crates/backends/tablecraft-access/Cargo.toml', 'tablecraft_access', ['lib']),
        ('apps/tablecraft-cli/Cargo.toml', 'tablecraft', ['bin']),
    ]
    for manifest, name, kind in owners:
        matches = [event for event in events
                   if event.get('reason') == 'compiler-artifact'
                   and event.get('manifest_path') == str(source_root / manifest)
                   and event['target']['name'] == name and event['target']['kind'] == kind]
        require(len(matches) == 1 and matches[0]['fresh'] is False,
                'Fresh compilation of exact exported engine/adapter/CLI owner')
        if kind == ['bin']:
            executable = str(historical_root / build['cli_artifact']['path'])
            require(matches[0]['executable'] == executable
                    and executable in matches[0]['filenames'], 'Actual compiled CLI executable')
    build_plan = load(build['build_plan'])
    require(build_plan['revision'] == record['producer_revision'], 'Build revision')
    for pin in build_plan['inputs']:
        check(pin)

    native = saved['native_receipt']
    sdk = saved['sdk_receipt']
    require(native['completed'] and native['failure'] is None
            and [native[key] for key in ['planned_commands', 'started_commands', 'completed_commands']]
            == [8, 8, 8] and (native['successes'], native['refusals']) == (8, 0),
            'Eight saved native calls')
    require(sdk['completed'] and sdk['failure'] is None and sdk['native_or_cli_producer_calls'] == 0
            and [sdk[key] for key in ['planned_children', 'started_children', 'completed_children']]
            == [8, 8, 8], 'Eight saved readonly getter calls')
    authorization = load(saved['native_plan']['manifest'])
    require(authorization['cases'] == expected['cases']
            and authorization['independent_expectations'] == evidence['expectations']
            and authorization['producer_revision'] == record['producer_revision'], 'Exact call authority')
    sdk_authorization = load(saved['sdk_plan']['manifest'])
    require(sdk_authorization['native_plan'] == evidence['native_plan']
            and sdk_authorization['native_receipt'] == evidence['native_receipt']
            and sdk_authorization['independent_expectations'] == evidence['expectations'],
            'Readonly calls bind completed native evidence')
    require(authorization['expected_source_log'] == evidence['expected_source_log'],
            'Expected-image admission before production')
    admission = read(evidence['expected_source_log'])
    require(b'qualify_genuine_preserved_text_growth_expectations_before_producer ... ok' in admission
            and b'1 passed; 0 failed' in admission, 'Eight expected images admitted before production')
    output_root = Path(evidence['native_plan']['path']).parent

    def derive_all():
        for header in [5, 6]:
            raw, getter, _ = originals[header]
            cases = [case for case in requests['cases'] if case['header'] == header]
            measured = next(item for item in saved['original_maps']['originals']
                            if item['header'] == header)
            yield from prepare.derive_cases(raw, getter, cases, measured)

    sql_calls = typed_calls = 0
    arrays = [requests['cases'], expected['cases'], native['calls'], saved['native_plan']['jobs'],
              sdk['calls'], saved['sdk_plan']['jobs'], derive_all()]
    for literal, case, call, job, reader, reader_job, derivation in zip(*arrays, strict=True):
        identifier, image, proof, reader_text = derivation
        require(identifier == literal['id'] == case['id'] == call['id'] == job['id']
                == reader['id'] == reader_job['id'], 'Exact ordered case identities')
        require(all(case[key] == literal[key]
                    for key in ['header', 'source', 'transport', 'sql', 'native_request']),
                'Frozen original request and caller order')
        if literal['transport'] == 'sql':
            sql_calls += 1
            require(isinstance(literal['sql'], str), 'Explicit literal SQL')
        else:
            typed_calls += 1
            require(literal['transport'] == 'typed' and literal['sql'] is None,
                    'Explicit typed batch without SQL ordering claim')
        saved_proof = load(case['proof'])
        require(all(saved_proof[key] == value for key, value in proof.items()), 'Independent row proof')
        require(read(case['expected']) == image and read(call['candidate']) == image
                and call['complete_image_equal'], 'Complete independent candidate equality')
        output = output_root / case['id'] / 'candidate.accdb'
        binary = str(historical_root / build['cli_artifact']['path'])
        source_path = str(historical_root / case['source']['path'])
        output_path = str(historical_root / output)
        if case['transport'] == 'sql':
            argv = [binary, 'query', 'preview-update', case['sql'], '--database', source_path,
                    '--output', output_path]
        else:
            argv = [binary, 'write', 'preview-update', source_path, output_path]
            request = literal['native_request']
            for update in request['updates']:
                def encode(value):
                    if value == {'kind': 'Null'}:
                        return 'null'
                    require(set(value) == {'kind', 'value'} and value['kind'] == 'Text',
                            'Exact typed Text or NULL carrier')
                    return 'value:' + value['value']

                argv.extend(['--set', request['table'], request['column'], str(update['ordinal']),
                             'text', encode(update['expected']), encode(update['replacement'])])
        argv.append('--variable')
        for flag, key in [('--work-bytes', 'bytes'), ('--work-rows', 'rows'), ('--work-steps', 'steps')]:
            argv.extend([flag, str(authorization['work_limits'][key])])
        require(call['argv'] == job['argv'] == argv
                and call['candidate']['path'] == output.as_posix(), 'Exact saved native argv/output')
        count = len(literal['native_request']['updates'])
        completion = REPORT_HEADER
        if case['transport'] == 'sql':
            completion += b'Matched rows\t' + str(count).encode() + b'\n'
        completion += b'Candidate rows\t' + str(count).encode() + b'\n'
        require(streams(call) == completion, 'Exact SQL or typed native report')
        require(saved_proof['expected_reader_stdout'] == case['reader']
                and read(case['reader']) == reader_text, 'Original-only reader projection')
        original_call = originals[case['header']][2]
        require(reader['argv'] == reader_job['argv'] == original_call['argv'][:-1] + [output_path]
                and reader['whole_inventory_equal']
                and reader_job['expected_reader_sha256'] == hashlib.sha256(reader_text).hexdigest(),
                'Exact original getter and actual candidate')
        require(streams(reader) == reader_text, 'Whole independent SDK inventory')
    require((sql_calls, typed_calls) == (4, 4), 'Four SQL singles and four typed batches')
    for pin in source['retained_originals']:
        check(pin)
    return {'saved_only': True, 'pins': len(checked), 'originals': 2, 'images': 8,
            'sql_calls': 4, 'typed_calls': 4, 'sdk_reads': 8, 'child_processes': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', default=str(ROOT))
    parser.add_argument('--record', default=DEFAULT_RECORD)
    args = parser.parse_args()
    print(json.dumps(verify(args.root, args.record), sort_keys=True))


if __name__ == '__main__':
    main()
