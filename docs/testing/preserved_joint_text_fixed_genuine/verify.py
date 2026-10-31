"""Verify saved joint fitting Text/Boolean evidence without launching a fixture or producer."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import tarfile

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'scripts'))
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'docs/testing'))
from compat_artifacts import ArtifactStore
from preserved_joint_text_fixed_genuine import model
from preserved_int32_genuine import framing

ROOT = Path(__file__).resolve().parents[3]
DEFAULT_RECORD = 'docs/testing/results/ace16-preserved-joint-text-fixed-genuine-images-2026-10-31.json'
REPORT_HEADER = b'Engineering copy\tExperimental; data macros and native Access saves unqualified\n'
WORK_LIMITS = {'bytes': 268435456, 'rows': 100000, 'steps': 100000000}
PRODUCER_REVISION = '510b4824d436e26c71c64c12306f198b2a6ef219'
require = model.require


SQL_EMPTY = (
    'UPDATE [PreservedRows] SET [Short_Text]=[Short_Text], '
    '[Flag]=[Flag] WHERE [ID]=-1'
)
PROFILES = ['beta-flag', 'text-noop-flag', 'null-flag', 'empty-null-flag',
            'disjoint-rows', 'all-noops', 'sql-empty']
CLI_KINDS = {'Short_Text': 'text', 'Flag': 'boolean'}


def literal_groups(profile, raw, original):
    """Independent exact request choices, with ordinals resolved from raw IDs."""
    fields = {field['name']: field for field in original['layout']['fields']}
    rows = {framing.integer(raw, row, fields['ID']): row for row in original['rows']}
    require(set(rows) == {1, 2, 3}, 'Exact three immutable original IDs')

    def text(value):
        return {'kind': 'Null'} if value is None else {'kind': 'Text', 'value': value}

    def boolean(value):
        return {'kind': 'Boolean', 'value': value}

    if profile == 'beta-flag':
        texts = [(1, text('alpha'), text('beta'))]
        flags = [(1, boolean(False), boolean(True))]
        payloads = {'1': 'fffe62657461'}
    elif profile == 'text-noop-flag':
        texts = [(1, text('alpha'), text('alpha'))]
        flags = [(1, boolean(False), boolean(True))]
        payloads = {'1': 'fffe616c706861'}
    elif profile == 'null-flag':
        texts = [(1, text('alpha'), text(None))]
        flags = [(1, boolean(False), boolean(True))]
        payloads = {'1': ''}
    elif profile == 'empty-null-flag':
        texts = [(2, text(None), text(''))]
        flags = [(2, boolean(True), boolean(False))]
        payloads = {'2': ''}
    elif profile == 'disjoint-rows':
        texts = [(1, text('alpha'), text('beta'))]
        flags = [(3, boolean(False), boolean(True))]
        payloads = {'1': 'fffe62657461'}
    elif profile == 'all-noops':
        texts = [(1, text('alpha'), text('alpha')), (2, text(None), text(None))]
        flags = [(1, boolean(False), boolean(False)), (2, boolean(True), boolean(True))]
        payloads = {'1': 'fffe616c706861', '2': ''}
    else:
        require(profile == 'sql-empty', 'Closed seven-profile domain')
        texts, flags, payloads = [], [], {}
    groups = []
    changed = set()
    for name, assignments in [('Short_Text', texts), ('Flag', flags)]:
        updates = []
        for identifier, before, after in assignments:
            row = rows[identifier]
            updates.append({'ordinal': row['ordinal'], 'row_id': {'ID': identifier},
                            'expected': before, 'replacement': after})
            if before != after:
                changed.add(row['reference'])
        groups.append({'column': name, 'updates': updates})
    return groups, payloads, sorted(changed)


def typed_value(value, column):
    model.token(value, column)
    if value == {'kind': 'Null'}:
        return 'null'
    literal = str(value['value']).lower() if column == 'Flag' else value['value']
    return 'value:' + literal


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
    require(record['producer_revision'] == PRODUCER_REVISION,
            'Exact reviewed committed functional producer')
    maintained = record['maintained_files']
    required_paths = {
        'scripts/compat_artifacts.py',
        'docs/testing/preserved_short_text_genuine/__init__.py',
        'docs/testing/preserved_short_text_genuine/framing.py',
        'docs/testing/preserved_joint_text_fixed_genuine/verify.py',
        'docs/testing/preserved_joint_text_fixed_genuine/__init__.py',
        'docs/testing/preserved_joint_text_fixed_genuine/model.py',
        'docs/testing/preserved_joint_text_fixed_genuine/recipes.py',
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
            and report['completed_cases'] == report['expected_images'] == 14
            and report['child_calls'] == report['writer_calls'] == 0,
            'Independent complete fourteen-image construction')
    require(report['expectations'] == evidence['expectations']
            and expected['request_inventory'] == evidence['requests'], 'Exact independent authority')
    require(expected['normal_save_or_journal_authority'] is False, 'Engineering-only evidence')
    inventory = load(expected['input_inventory'])
    require(inventory['schema'] == 1 and inventory['state'] == 'BOUND'
            and inventory['requests'] == evidence['requests'], 'Bound construction input wrapper')
    for pin in inventory['inputs']:
        check(pin)
    closure = {(pin['path'], pin['bytes'], pin['sha256']) for pin in inventory['inputs']}
    closure.add(tuple(expected['input_inventory'][key] for key in ['path', 'bytes', 'sha256']))
    controller_path = Path(expected['input_inventory']['path']).with_name('prepare_expected.py').as_posix()
    controllers = [pin for pin in expected['inputs'] if pin['path'] == controller_path]
    require(len(controllers) == 1, 'One saved pure construction controller')
    closure.add(tuple(controllers[0][key] for key in ['path', 'bytes', 'sha256']))
    actual_closure = {(pin['path'], pin['bytes'], pin['sha256']) for pin in expected['inputs']}
    require(actual_closure == closure, 'Complete actual construction closure')
    frozen = load(inventory['original_freeze'])
    require(frozen['cases'] == 14 and frozen['sources'] == 2 and len(frozen['files']) == 49
            and frozen['child_calls'] == frozen['writer_calls'] == 0,
            'Original unexecuted fourteen-case input freeze retained')
    historical = {pin['path']: pin for pin in inventory['historical_tools']}
    require(set(historical) == {'/usr/bin/java', '/usr/bin/javac'}
            and expected['historical_tools'] == inventory['historical_tools'],
            'Exact two historical external tool observations, never reopened by saved verifier')
    source_tools = {tool['path']: {key: tool[key] for key in ['path', 'bytes', 'sha256']}
                    for tool in saved['source_receipt']['tools']}
    require(historical == source_tools, 'Historical tool observations match actual SOURCE5 receipt')
    for pin in frozen['files']:
        if Path(pin['path']).is_absolute():
            require(historical.get(pin['path']) == pin, 'Closed historical-tool metadata observation')
        else:
            check(pin)
            require((pin['path'], pin['bytes'], pin['sha256']) in closure,
                    'Every original root-scoped input retained in construction closure')
    for name in ['model', 'recipes']:
        public = next(pin for pin in maintained if pin['path']
                      == f'docs/testing/preserved_joint_text_fixed_genuine/{name}.py')
        require(read(inventory[name]) == read(public), 'Exact executed pure owner promotion')
    require(inventory['source_receipt'] == requests['source_receipt'] == evidence['source_receipt']
            and inventory['source_execution_plan'] == requests['source_execution_plan']
            == evidence['source_execution_plan']
            and inventory['native_source_log'] == evidence['native_source_log'],
            'Actual construction wrapper binds original receipt and fresh admission')
    historical_admission = read(requests['native_source_admission'])
    require(b'qualify_genuine_preserved_short_text_originals_before_expected_or_producer ... ok'
            in historical_admission and b'1 passed; 0 failed' in historical_admission,
            'Historical original Source admission remains distinct from the fresh gate')
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
            == [5, 5, 5], 'Five original-only compile/create/inspect calls')
    require(source['source_plan'] == execution['source_plan'] == evidence['source_plan']
            and execution['commands'] == source_plan['commands'], 'Exact executed source plan')
    require(source['tools'] == execution['tools']
            and [tool['command'] for tool in source['tools']] == ['javac', 'java'],
            'Same recorded tool identities throughout original execution')
    require([call['id'] for call in source['calls']]
            == ['compile', 'create-h5', 'create-h6', 'inspect-h5', 'inspect-h6'],
            'Closed ordered original-only calls')
    require([call['argv'] for call in source['calls']]
            == [command['argv'] for command in source_plan['commands']], 'Exact original-only argv')
    for call in source['calls']:
        stdout = streams(call)
        if call['id'] == 'compile' or call['id'].startswith('create-'):
            require(not stdout, 'Compilation and original creation stdout empty')
        for pin in call['originals_before'] + call['originals_after']:
            check(pin)
    for pin in source['inputs']:
        check(pin)
    require(source['calls'][0]['originals_before'] == source['calls'][0]['originals_after'] == [],
            'Compilation precedes all generated originals')
    classes = source['retained_classes'] + source_plan['existing_classes_unchanged']
    require(len(classes) == 3 and {Path(pin['path']).name for pin in classes}
            == {'PreservedShortTextOriginals.class', 'PreservedInt32Originals.class',
                'BoundedFileChannel.class'}, 'Exact original generator and unchanged getter classes')
    directories = {}
    for pin in classes:
        check(pin)
        directories.setdefault(Path(pin['path']).parent, set()).add(Path(pin['path']).name)
    for directory, expected_names in directories.items():
        actual_names = {path.relative_to(store.path(directory)).as_posix()
                        for path in store.path(directory).rglob('*') if path.is_file()}
        require(actual_names == expected_names, 'Complete unchanged class-directory membership')
    original_pins = source['retained_originals']
    require(len(original_pins) == 2, 'Exactly two retained original images')
    for index, header in enumerate([5, 6]):
        pin = original_pins[index]
        require(pin['path'] == str(Path(source_plan['work']) / 'sources' / f'h{header}.accdb'),
                'Exact original output within isolated source namespace')
        create_call = source['calls'][index + 1]
        getter_call = source['calls'][index + 3]
        require(create_call['argv'][-3:] == ['PreservedShortTextOriginals', pin['path'],
                                           'V2016' if header == 5 else 'V2019'],
                'Original creation command has no update oracle')
        require(getter_call['argv'][-3:] == ['PreservedInt32Originals', '--inspect', pin['path']],
                'Exact unchanged readonly original getter')
        require(create_call['originals_before'] == original_pins[:index]
                and create_call['originals_after'] == original_pins[:index + 1]
                and getter_call['originals_before'] == getter_call['originals_after'] == original_pins,
                'Every earlier original retained through every source call')
    originals = {}
    for pin in original_pins:
        raw = read(pin)
        header = raw[20]
        require(header in [5, 6] and header not in originals, 'Distinct original headers')
        call = next(call for call in source['calls'] if call['id'] == f'inspect-h{header}')
        getter = streams(call).decode('utf-8')
        originals[header] = pin, raw, getter, call, model.qualify(raw, getter, header)
    require(set(originals) == {5, 6}, 'Both immutable originals')
    admission = read(evidence['native_source_log'])
    require(b'qualify_genuine_preserved_short_text_originals_before_expected_or_producer ... ok'
            in admission and b'1 passed; 0 failed' in admission, 'Actual original Source admission')
    admission = read(evidence['expected_source_log'])
    require(b'qualify_genuine_preserved_joint_text_fixed_expectations_before_producer ... ok'
            in admission and b'1 passed; 0 failed' in admission, 'Actual fourteen-image Source admission')

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
            == [14, 14, 14] and (native['successes'], native['refusals']) == (14, 0)
            and native['sdk_calls'] == 0,
            'Fourteen completed native calls')
    require(sdk['completed'] and sdk['failure'] is None and sdk['native_or_cli_producer_calls'] == 0
            and [sdk[key] for key in ['planned_children', 'started_children', 'completed_children']]
            == [14, 14, 14], 'Fourteen readonly getter calls')
    authorization = load(saved['native_plan']['manifest'])
    require(authorization['cases'] == expected['cases']
            and authorization['independent_expectations'] == evidence['expectations']
            and authorization['producer_revision'] == record['producer_revision']
            and authorization['expected_source_log'] == evidence['expected_source_log']
            and authorization['work_limits'] == WORK_LIMITS, 'Exact native call authority')
    for name in ['build_receipt', 'export_receipt', 'source_receipt', 'source_execution_plan', 'native_source_log']:
        require(authorization[name] == evidence[name], 'Native authority binds actual ' + name)
    require(authorization['expected_construction_receipt'] == evidence['expected_report'],
            'Native authority follows actual independent construction')
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
                'Exact ordered fourteen-case identities')
        require(all(case[key] == value for key, value in request_case.items()),
                'Immutable original joint request binding')
        source_pin, raw, getter, original_call, qualified = originals[header]
        require(case['source'] == source_pin and request_case['sdk_stdout'] == original_call['logs'][0],
                'Exact original bytes and complete getter observations')
        groups, payloads, roots = literal_groups(profile, raw, qualified)
        require(request_case['literal_text_payloads'] == payloads, 'Independent literal Text encoding choices')
        require(case['native_request'] == {'table': 'PreservedRows', 'groups': groups},
                'Independent exact field/update order, original values, ordinals and replacement')
        count = len(roots)
        require(request_case['expected_changed_rows'] == count
                and case['expected_outcome'] == {'kind': 'candidate', 'changed_rows': count},
                'Independent original-root union, not changed-cell sum')
        if profile == 'sql-empty':
            require(case['transport'] == 'sql' and case['sql'] == SQL_EMPTY,
                    'Exact Text/Flag no-match SQL with true empty groups')
            sql_calls += 1
        else:
            require(case['transport'] == 'typed' and case['sql'] is None,
                    'Twelve exact typed batches, without fabricated empty assignments')
            typed_calls += 1
        image, proof, reader_text = model.construct(raw, qualified, request_case)
        require(proof['changed_root_references'] == roots, 'Actual raw logical-root union proof')
        saved_proof = load(case['proof'])
        require(all(saved_proof[key] == value for key, value in proof.items()),
                'Complete independently reconstructed joint preservation proof')
        require(read(case['expected']) == image and read(call['candidate']) == image
                and call['complete_image_equal'], 'Whole independent candidate image/EOF equality')
        require(saved_proof['reader_sha256'] == case['reader']['sha256']
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
                cli_kind = CLI_KINDS[group['column']]
                require(group['updates'], 'Typed transport contains actual nonempty groups')
                for update in group['updates']:
                    argv.extend(['--set', 'PreservedRows', group['column'], str(update['ordinal']),
                                 cli_kind, typed_value(update['expected'], group['column']),
                                 typed_value(update['replacement'], group['column'])])
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
    require((typed_calls, sql_calls, changed_images, noops, changed_roots) == (12, 2, 10, 4, 12),
            'Fourteen exact calls, ten changed images, four noops and twelve changed roots')
    for pin in source['retained_originals'] + classes + maintained:
        check(pin)
    return {'saved_only': True, 'pins': len(checked), 'originals': 2, 'images': 14,
            'typed_calls': 12, 'sql_calls': 2, 'changed_images': 10, 'noops': 4,
            'changed_roots': 12, 'sdk_reads': 14, 'child_processes': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', default=str(ROOT))
    parser.add_argument('--record', default=DEFAULT_RECORD)
    args = parser.parse_args()
    print(json.dumps(verify(args.root, args.record), sort_keys=True))


if __name__ == '__main__':
    main()
