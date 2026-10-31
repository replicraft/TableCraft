"""Verify saved joint fixed evidence without launching a fixture or producer."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import tarfile

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'scripts'))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from compat_artifacts import ArtifactStore
from preserved_joint_fixed_genuine import model
from preserved_int32_genuine import framing

ROOT = Path(__file__).resolve().parents[3]
DEFAULT_RECORD = 'docs/testing/results/ace16-preserved-joint-fixed-genuine-images-2026-10-31.json'
REPORT_HEADER = b'Engineering copy\tExperimental; data macros and native Access saves unqualified\n'
WORK_LIMITS = {'bytes': 268435456, 'rows': 100000, 'steps': 100000000}
PRODUCER_REVISION = '74b6b112a7ed86440d0e2d5f6029292f15c48128'
require = model.require


SQL_EMPTY = (
    'UPDATE [PreservedRows] SET [Flag]=[Flag], [Large_Number]=[Large_Number], '
    '[Date_Field]=[Date_Field], [Time_Field]=[Time_Field], '
    '[Currency_Field]=[Currency_Field] WHERE [ID]=-1'
)
CLI_KINDS = {'Large_Number': ('Int64', 'int64'),
             'Currency_Field': ('Currency', 'currency-scaled'),
             'Date_Field': ('DateTime', 'date-bits'),
             'Time_Field': ('DateTime', 'date-bits'), 'Flag': ('Boolean', 'boolean')}
PROFILES = ['same-row', 'promote-null', 'clear-values', 'overlap-rows', 'mixed-noops', 'sql-empty']


def literal_groups(profile, raw, original):
    """Independent literal law; actual raw ID lookup supplies every ordinal."""
    rows = {framing.integer(raw, row, original['fields']['ID']): row for row in original['rows']}
    require(set(rows) == {1, 2, 3}, 'Three actual original ID bindings')
    null = {'kind': 'Null'}

    def integer(number):
        return {'kind': 'Int64', 'value': number}

    def currency(number):
        return {'kind': 'Currency', 'scaled': number}

    def date(bits):
        return {'kind': 'DateTime', 'bits': bits}

    def boolean(flag):
        return {'kind': 'Boolean', 'value': flag}

    low, high = -9223372036854775808, 9223372036854775807
    zero = date('0000000000000000')
    quarter = date('3fd0000000000000')
    half = date('3fe0000000000000')
    if profile == 'same-row':
        specifications = [
            ('Large_Number', [(1, integer(1000), integer(low))]),
            ('Currency_Field', [(1, currency(12345), currency(low))]),
            ('Date_Field', [(1, zero, date('8000000000000000'))]),
            ('Time_Field', [(1, half, quarter)]),
            ('Flag', [(1, boolean(False), boolean(True))]),
        ]
    elif profile == 'promote-null':
        specifications = [
            ('Large_Number', [(2, null, integer(high))]),
            ('Currency_Field', [(2, null, currency(high))]),
            ('Date_Field', [(2, null, quarter)]),
            ('Time_Field', [(2, null, half)]),
            ('Flag', [(2, boolean(True), boolean(False))]),
        ]
    elif profile == 'clear-values':
        specifications = [
            ('Large_Number', [(1, integer(1000), null)]),
            ('Currency_Field', [(1, currency(12345), null)]),
            ('Date_Field', [(1, zero, null)]),
            ('Time_Field', [(1, half, null)]),
            ('Flag', [(1, boolean(False), boolean(True))]),
        ]
    elif profile == 'overlap-rows':
        specifications = [
            ('Large_Number', [(1, integer(1000), integer(-101)), (2, null, integer(high))]),
            ('Currency_Field', [(1, currency(12345), null), (2, null, currency(-10001))]),
            ('Date_Field', [(2, null, quarter), (3, date('40e624e000000000'), null)]),
            ('Time_Field', [(1, half, quarter), (3, quarter, half)]),
            ('Flag', [(1, boolean(False), boolean(True)), (2, boolean(True), boolean(False)),
                      (3, boolean(False), boolean(True))]),
        ]
    elif profile == 'mixed-noops':
        specifications = [
            ('Large_Number', [(2, null, null)]), ('Currency_Field', [(2, null, null)]),
            ('Date_Field', [(1, zero, zero)]), ('Time_Field', [(1, half, half)]),
            ('Flag', [(2, boolean(True), boolean(True))]),
        ]
    else:
        require(profile == 'sql-empty', 'Closed six-profile domain')
        specifications = [(name, []) for name in
                          ['Flag', 'Large_Number', 'Date_Field', 'Time_Field', 'Currency_Field']]
    groups = []
    changed_roots = set()
    for name, assignments in specifications:
        updates = []
        for identifier, before, after in assignments:
            row = rows[identifier]
            updates.append({'ordinal': row['ordinal'], 'row_id': {'ID': identifier},
                            'expected': before, 'replacement': after})
            if before != after:
                changed_roots.add(row['reference'])
        groups.append({'column': name, 'updates': updates})
    return groups, sorted(changed_roots)


def typed_value(value, kind):
    model.encoded(value, kind)
    if value == {'kind': 'Null'}:
        return 'null'
    key = {'Int64': 'value', 'Currency': 'scaled', 'DateTime': 'bits', 'Boolean': 'value'}[kind]
    token = str(value[key]).lower() if kind == 'Boolean' else str(value[key])
    return 'value:' + token


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
    require(record['producer_revision'] == PRODUCER_REVISION, 'Exact committed functional producer')
    maintained = record['maintained_files']
    required_paths = {
        'scripts/compat_artifacts.py',
        'docs/testing/preserved_fixed_scalars_genuine/__init__.py',
        'docs/testing/preserved_fixed_scalars_genuine/model.py',
        'docs/testing/preserved_joint_fixed_genuine/verify.py',
        'docs/testing/preserved_joint_fixed_genuine/__init__.py',
        'docs/testing/preserved_joint_fixed_genuine/model.py',
        'docs/testing/preserved_int32_genuine/__init__.py',
        'docs/testing/preserved_int32_genuine/framing.py',
        'docs/testing/ace16_insert_candidates/model.py',
    }
    require(required_paths <= {pin['path'] for pin in maintained}, 'Pinned maintained import closure')
    for pin in maintained:
        check(pin)
    for pin in record['phase_inputs']:
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
            and report['completed_cases'] == report['expected_images'] == 12
            and report['child_calls'] == report['writer_calls'] == 0,
            'Independent complete twelve-image construction')
    require(report['expectations'] == evidence['expectations']
            and expected['request_inventory'] == evidence['requests'], 'Exact independent authority')
    require(expected['normal_save_or_journal_authority'] is False, 'Engineering-only evidence')
    inventory = load(expected['input_inventory'])
    require(expected['inputs'][0] == expected['input_inventory']
            and expected['inputs'][2:] == inventory['inputs']
            and inventory['requests'] == evidence['requests'],
            'Complete original-only construction closure')
    public_model = next(pin for pin in maintained
                        if pin['path'] == 'docs/testing/preserved_joint_fixed_genuine/model.py')
    require(read(inventory['model']) == read(public_model), 'Exact executed pure model promotion')
    require(requests['source_receipt'] == evidence['source_receipt']
            and requests['source_execution_plan'] == evidence['source_execution_plan']
            and requests['native_source_admission'] == evidence['native_source_log'],
            'Requests bind actual original admission before construction')
    for pin in expected['inputs']:
        check(pin)
    for name in ['source_plan', 'source_execution_plan', 'native_plan', 'sdk_plan']:
        for pin in saved[name]['inputs']:
            check(pin)

    source = saved['source_receipt']
    source_plan = saved['source_plan']
    execution = saved['source_execution_plan']
    require(source['completed'] and source['failure'] is None
            and source['native_or_cli_producer_calls'] == 0
            and [source[key] for key in ['planned_children', 'started_children', 'completed_children']]
            == [4, 4, 4], 'Four original-only source calls')
    require(source['source_plan'] == execution['source_plan'] == evidence['source_plan']
            and execution['commands'] == source_plan['commands'], 'Exact executed source plan')
    require(source['tools'] == execution['tools']
            and [tool['command'] for tool in source['tools']] == ['java'],
            'Same recorded tool identities throughout source execution')
    require([call['id'] for call in source['calls']]
            == ['create-h5', 'create-h6', 'inspect-h5', 'inspect-h6'], 'Closed ordered source calls')
    require([call['argv'] for call in source['calls']]
            == [command['argv'] for command in source_plan['commands']], 'Exact original-only argv')
    for call in source['calls']:
        stdout = streams(call)
        if call['id'].startswith('create-'):
            require(not stdout, 'Creation stdout empty')
        for pin in call['originals_before'] + call['originals_after']:
            check(pin)
    for pin in source['inputs']:
        check(pin)

    compile_plan = saved['source_compile_plan']
    require(saved['source_compile_result'] == {'exit_code': 0}, 'Successful saved javac result')
    require(not read(evidence['source_compile_stdout'])
            and not read(evidence['source_compile_stderr']), 'Empty javac streams')
    classes = source['retained_classes']
    class_names = ['BoundedFileChannel.class', 'PreservedFixedScalarGetter.class',
                   'PreservedFixedScalarOriginals.class']
    require([Path(pin['path']).name for pin in classes] == class_names,
            'Exact three compiled source/getter classes')
    class_directory = Path(classes[0]['path']).parent
    actual_classes = sorted(path.relative_to(store.path(class_directory)).as_posix()
                            for path in store.path(class_directory).rglob('*') if path.is_file())
    require(actual_classes == class_names
            and all(Path(pin['path']).parent == class_directory for pin in classes),
            'Complete unchanged class directory inventory')
    for pin in classes:
        check(pin)
    copies = compile_plan['inputs']
    require(len(copies) == 4, 'Three Java sources and one JAR')
    for pin in copies:
        require(read({'path': pin['source'], 'bytes': pin['bytes'], 'sha256': pin['sha256']})
                == read({'path': pin['copy'], 'bytes': pin['bytes'], 'sha256': pin['sha256']}),
                'Actual source/JAR copies retained')
    by_name = {Path(pin['copy']).name: pin['copy'] for pin in copies}
    require(set(by_name) == {'BoundedFileChannel.java', 'PreservedFixedScalarGetter.java',
                             'PreservedFixedScalarOriginals.java', 'jackcess-5.0.4.jar'},
            'Exact compilation inputs')
    require(compile_plan['argv'] == [
        'javac', '-J-Xmx512m', '-encoding', 'UTF-8', '-cp', by_name['jackcess-5.0.4.jar'],
        '-d', class_directory.as_posix(), by_name['PreservedFixedScalarOriginals.java'],
        by_name['PreservedFixedScalarGetter.java'], by_name['BoundedFileChannel.java']],
        'Exact bounded javac command')
    java_prefix = [
        'java', '-Xmx512m', '-Dfile.encoding=UTF-8', '-Dstdout.encoding=UTF-8',
        '-Dstderr.encoding=UTF-8', '-Duser.timezone=UTC', '-Duser.language=en',
        '-Duser.country=US', '-cp',
        by_name['jackcess-5.0.4.jar'] + ':' + class_directory.as_posix(),
    ]
    original_pins = source['retained_originals']
    require(len(original_pins) == 2, 'Exactly two retained originals')
    for index, header in enumerate([5, 6]):
        pin = original_pins[index]
        require(pin['path'] == str(Path(source_plan['work']) / 'sources' / f'h{header}.accdb'),
                'Exact original output within isolated source namespace')
        create_call = source['calls'][index]
        getter_call = source['calls'][index + 2]
        require(create_call['argv'] == java_prefix + [
            'PreservedFixedScalarOriginals', pin['path'], 'V2016' if header == 5 else 'V2019'],
            'Exact creation-only original command')
        require(getter_call['argv'] == java_prefix + [
            'PreservedFixedScalarGetter', '--inspect', pin['path']], 'Exact readonly original getter')
        require(create_call['originals_before'] == original_pins[:index]
                and create_call['originals_after'] == original_pins[:index + 1]
                and getter_call['originals_before'] == getter_call['originals_after'] == original_pins,
                'Creation and getter calls retain every earlier original')

    originals = {}
    for pin in source['retained_originals']:
        raw = read(pin)
        header = raw[20]
        require(header in [5, 6] and header not in originals, 'Distinct original headers')
        call = next(call for call in source['calls'] if call['id'] == f'inspect-h{header}')
        require(call['argv'][-3:] == ['PreservedFixedScalarGetter', '--inspect', pin['path']],
                'Exact original readonly getter')
        getter = streams(call).decode('utf-8')
        originals[header] = pin, raw, getter, call, model.qualify(raw, getter, header)
    require(set(originals) == {5, 6}, 'Both immutable originals')
    admission = read(evidence['native_source_log'])
    require(b'qualify_genuine_preserved_fixed_scalar_originals_before_expected_or_producer ... ok'
            in admission and b'1 passed; 0 failed' in admission, 'Actual original Source admission')
    admission = read(evidence['expected_source_log'])
    require(b'qualify_genuine_preserved_joint_fixed_expectations_before_producer ... ok'
            in admission and b'1 passed; 0 failed' in admission, 'Actual expected Source admission')

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
    target_directory = historical_root / Path(build['cli_artifact']['path']).parent.parent
    require(build_plan['command'] == [
        'cargo', 'build', '-p', 'tablecraft-cli', '--bin', 'tablecraft', '--release',
        '--locked', '--offline', '-j2', '--target-dir', str(target_directory),
        '--message-format=json'], 'Exact dedicated locked offline release command')
    require(build_plan['target_initially_absent'] and not build_plan['profile_overrides']
            and not build_plan['incremental_override']
            and build_plan['inherited_compiler_profile_environment'] == {}
            and build_plan['compiler_wrapper_environment']
            == {'RUSTC_WRAPPER': '', 'RUSTC_WORKSPACE_WRAPPER': ''}
            and build_plan['producer_invocations'] == 0,
            'Fresh target, unchanged build profile and no producer invocation during build')
    for pin in build_plan['inputs']:
        check(pin)
    native = saved['native_receipt']
    sdk = saved['sdk_receipt']
    require(native['completed'] and native['failure'] is None
            and [native[key] for key in ['planned_commands', 'started_commands', 'completed_commands']]
            == [12, 12, 12] and (native['successes'], native['refusals']) == (12, 0)
            and native['sdk_calls'] == 0,
            'Twelve completed native calls')
    require(sdk['completed'] and sdk['failure'] is None and sdk['native_or_cli_producer_calls'] == 0
            and [sdk[key] for key in ['planned_children', 'started_children', 'completed_children']]
            == [12, 12, 12], 'Twelve readonly getter calls')
    authorization = load(saved['native_plan']['manifest'])
    require(authorization['cases'] == expected['cases']
            and authorization['independent_expectations'] == evidence['expectations']
            and authorization['producer_revision'] == record['producer_revision']
            and authorization['expected_source_log'] == evidence['expected_source_log']
            and authorization['work_limits'] == WORK_LIMITS, 'Exact native call authority')
    for name in ['build_receipt', 'export_receipt', 'source_receipt', 'native_source_log']:
        require(authorization[name] == evidence[name], 'Native authority binds actual ' + name)
    require(authorization['expected_construction_receipt'] == evidence['expected_report']
            and authorization['compile_receipt'] == evidence['source_compile_result'],
            'Native authority follows actual independent construction and javac')
    sdk_authorization = load(saved['sdk_plan']['manifest'])
    require(sdk_authorization['native_plan'] == evidence['native_plan']
            and sdk_authorization['native_receipt'] == evidence['native_receipt']
            and sdk_authorization['independent_expectations'] == evidence['expectations']
            and sdk_authorization['native_manifest'] == saved['native_plan']['manifest']
            and sdk_authorization['source_receipt'] == evidence['source_receipt']
            and sdk_authorization['source_execution_plan'] == evidence['source_execution_plan'],
            'Readonly authority binds completed native images')
    output_root = Path(evidence['native_plan']['path']).parent
    literals = [(f'h{header}-{profile}', header, profile) for header in [5, 6] for profile in PROFILES]
    arrays = [literals, requests['cases'], expected['cases'], native['calls'],
              saved['native_plan']['jobs'], sdk['calls'], saved['sdk_plan']['jobs']]
    changed_images = noops = changed_roots = typed_calls = sql_calls = 0
    for literal, request_case, case, call, job, reader, reader_job in zip(*arrays, strict=True):
        identifier, header, profile = literal
        require(identifier == request_case['id'] == case['id'] == call['id'] == job['id']
                == reader['id'] == reader_job['id'] and case['header'] == header,
                'Exact ordered twelve-case identities')
        require(all(case[key] == request_case[key]
                    for key in ['header', 'source', 'transport', 'sql', 'native_request']),
                'Immutable original joint request binding')
        source_pin, raw, getter, original_call, qualified = originals[header]
        require(case['source'] == source_pin and request_case['sdk_stdout'] == original_call['logs'][0],
                'Exact original bytes and complete getter observations')
        groups, roots = literal_groups(profile, raw, qualified)
        require(case['native_request'] == {'table': 'PreservedRows', 'groups': groups},
                'Independent exact field/update order, original values, ordinals and replacement')
        count = len(roots)
        require(request_case['expected_changed_rows'] == count
                and case['expected_outcome'] == {'kind': 'candidate', 'changed_rows': count},
                'Independent original-root union, not changed-cell sum')
        if profile == 'sql-empty':
            require(case['transport'] == 'sql' and case['sql'] == SQL_EMPTY,
                    'Exact Flag-first no-match SQL with true empty groups')
            sql_calls += 1
        else:
            require(case['transport'] == 'typed' and case['sql'] is None,
                    'Ten exact typed batches, without fabricated empty assignments')
            typed_calls += 1
        image, proof, reader_text = model.construct(raw, qualified, request_case)
        require(proof['changed_root_references'] == roots, 'Actual raw logical-root union proof')
        saved_proof = load(case['proof'])
        require(all(saved_proof[key] == value for key, value in proof.items()),
                'Complete independently reconstructed joint preservation proof')
        require(read(case['expected']) == image and read(call['candidate']) == image
                and call['complete_image_equal'], 'Whole independent candidate image/EOF equality')
        require(saved_proof['expected_reader_stdout'] == case['reader']
                and read(case['reader']) == reader_text, 'Original-only whole getter expectation')
        if count == 0:
            require(image == raw, 'Noop and SQL-empty images preserve every original byte')
        output = output_root / identifier / 'candidate.accdb'
        binary = str(historical_root / build['cli_artifact']['path'])
        source_path = str(historical_root / source_pin['path'])
        output_path = str(historical_root / output)
        if profile == 'sql-empty':
            argv = [binary, 'query', 'preview-update', SQL_EMPTY, '--database', source_path,
                    '--output', output_path]
        else:
            argv = [binary, 'write', 'preview-update', source_path, output_path]
            for group in groups:
                kind, cli_kind = CLI_KINDS[group['column']]
                require(group['updates'], 'Typed transport contains actual nonempty groups')
                for update in group['updates']:
                    argv.extend(['--set', 'PreservedRows', group['column'], str(update['ordinal']),
                                 cli_kind, typed_value(update['expected'], kind),
                                 typed_value(update['replacement'], kind)])
        for flag, key in [('--work-bytes', 'bytes'), ('--work-rows', 'rows'), ('--work-steps', 'steps')]:
            argv.extend([flag, str(WORK_LIMITS[key])])
        require(call['argv'] == job['argv'] == argv
                and call['candidate']['path'] == output.as_posix(), 'Exact native argv and fresh output')
        completion = REPORT_HEADER
        if profile == 'sql-empty':
            completion += b'Matched rows\t0\n'
        completion += f'Candidate rows\t{count}\n'.encode('ascii')
        require(streams(call) == completion, 'Exact typed or SQL joint completion report')
        require(reader['argv'] == reader_job['argv'] == original_call['argv'][:-1] + [output_path]
                and reader['whole_inventory_equal']
                and reader_job['expected_reader_sha256'] == hashlib.sha256(reader_text).hexdigest(),
                'Exact original getter prefix and actual candidate input')
        require(streams(reader) == reader_text, 'Whole ordered SDK observation equality')
        changed_images += int(count != 0)
        noops += int(count == 0)
        changed_roots += count
    require((typed_calls, sql_calls, changed_images, noops, changed_roots) == (10, 2, 8, 4, 12),
            'Twelve exact calls, eight changed images, four noops and twelve changed roots')
    for pin in source['retained_originals'] + classes + maintained:
        check(pin)
    return {'saved_only': True, 'pins': len(checked), 'originals': 2, 'images': 12,
            'typed_calls': 10, 'sql_calls': 2, 'changed_images': 8, 'noops': 4,
            'changed_roots': 12, 'sdk_reads': 12, 'child_processes': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', default=str(ROOT))
    parser.add_argument('--record', default=DEFAULT_RECORD)
    args = parser.parse_args()
    print(json.dumps(verify(args.root, args.record), sort_keys=True))


if __name__ == '__main__':
    main()
