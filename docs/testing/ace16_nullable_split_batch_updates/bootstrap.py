"""Load maintained program bytes with their original, evidence-root namespaces.

The original cached files are hash witnesses only. They are never imported or
executed. Virtual __file__ values reproduce the executed ROOT/HERE semantics;
actual shared raw/frame owners are imported from the maintained repository.
"""
from contextlib import contextmanager
import hashlib
import importlib
from pathlib import Path
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
PUBLIC = ROOT / 'docs/testing/ace16_nullable_split_batch_updates'


def identity(store, pins, anchors):
    for item in anchors['programs']:
        original = pins.check(item['original']).read_bytes()
        maintained = (ROOT / item['maintained']).read_bytes()
        assert maintained == original, item['maintained']
    public = (PUBLIC / 'executed/reference/reference.py').read_text()
    native = (PUBLIC / 'executed/native/proof.py').read_text()
    original_stream = '''    intent.check_streams(Path(action_stdout).read_bytes(), Path(action_stderr).read_bytes(),
                         case, observed)
'''
    native_stream = '''    changed = sum(values != original['values'][identity]
                  for identity, values in observed['rows'].items())
    assert Path(action_stdout).read_bytes() == f'candidate_changed_rows={changed}\\n'.encode()
    assert Path(action_stderr).read_bytes() == (
        b'Engineering candidate: delete actions, data-macro semantics and native Access qualification pending.\\n'
    )
'''
    expected = public.replace(public.splitlines()[0], native.splitlines()[0], 1)
    assert expected.count(original_stream) == 1
    expected = expected.replace(original_stream, native_stream)
    expected = expected.replace("updates=case['updates'], warnings=len(observed['warnings']),",
                                "updates=case['updates'], public_warning_intent=len(observed['warnings']),")
    assert native == expected, 'Unreviewed native raw-proof change'
    return len(anchors['programs'])


@contextmanager
def load(store, pins, anchors, frozen_inputs, native=False):
    programs = {(item['phase'], Path(item['maintained']).name): item
                for item in anchors['programs']}
    saved_path = list(sys.path)
    aliases = ['source', 'source_warnings', 'model', 'check', 'intent', 'reference']
    if native:
        aliases.append('native_proof')
    previous = {name: sys.modules.get(name) for name in aliases}
    loaded = {}
    try:
        # Validate shared owner bytes before their top-level imports execute.
        frozen = {item['path']: item for item in frozen_inputs}
        for name, item in frozen.items():
            if name.startswith('docs/testing/') and name.endswith('.py'):
                pins.check(item)
                raw = (ROOT / name).read_bytes()
                assert len(raw) == item['bytes']
                assert hashlib.sha256(raw).hexdigest() == item['sha256'], name
        sys.path.insert(0, str(ROOT / 'docs/testing'))
        old_modules = set(sys.modules)
        importlib.import_module('ace16_fixed_ignore_null_updates.source')
        importlib.import_module('ace16_insert_multiple_indexes.model')
        for name in set(sys.modules) - old_modules:
            origin = getattr(sys.modules[name], '__file__', None)
            if origin is None:
                continue
            path = Path(origin).resolve()
            try:
                relative = path.relative_to(ROOT).as_posix()
            except ValueError:
                continue
            if relative.startswith('docs/testing/') and path.suffix == '.py':
                assert relative in frozen, ('Unbound shared owner', relative)

        selection = [('source', 'source.py', 'source'),
                     ('source', 'source_warnings.py', 'source_warnings'),
                     ('expected', 'model.py', 'model'),
                     ('expected', 'check.py', 'check'),
                     ('reference', 'intent.py', 'intent'),
                     ('reference', 'reference.py', 'reference')]
        if native:
            selection.append(('native', 'proof.py', 'native_proof'))
        for phase, filename, alias in selection:
            item = programs[phase, filename]
            raw = (ROOT / item['maintained']).read_bytes()
            assert raw == pins.check(item['original']).read_bytes()
            origin = store.path(item['original']['path'])
            module = types.ModuleType('ace16_nullable_split_batch_updates.replayed_' + alias)
            module.__file__ = str(origin)
            module.__package__ = ''
            sys.modules[alias] = module
            exec(compile(raw, str(origin), 'exec'), module.__dict__)
            loaded[alias] = module
        yield types.SimpleNamespace(**loaded)
    finally:
        sys.path[:] = saved_path
        for name, module in previous.items():
            if module is None:
                sys.modules.pop(name, None)
            else:
                sys.modules[name] = module
