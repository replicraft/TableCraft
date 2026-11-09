"""Pure source-witness mapping tests; no database, build or child processes."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

VERIFIER = Path(__file__).resolve().parents[1] / 'verify_data_macro_xml.py'
SPEC = importlib.util.spec_from_file_location('data_macro_xml_current_verifier', VERIFIER)
verifier = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(verifier)


def pin(path, raw):
    return {'path': path, 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}


class SourceWitnessTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.store = verifier.ArtifactStore(self.root)
        self.original = pin('engine/source.rs', b'historical Rust source\n')
        # This witness would have an effect if somebody incorrectly imported it.
        python = b"raise RuntimeError('historical witness must only be read')\n"
        self.python = pin('tools/historical.py', python)
        self.record = {'source_files': [self.original, self.python]}
        raw = self.json_bytes(self.record)
        self.record_pin = pin(verifier.DEFAULT_RECORD, raw)
        self.write(verifier.DEFAULT_RECORD, raw)
        self.manifest = {
            'record': self.record_pin,
            'source_witnesses': [self.entry(self.original), self.entry(self.python)],
        }
        self.write(self.entry(self.original)['witness']['path'], b'historical Rust source\n')
        self.write(self.entry(self.python)['witness']['path'], python)
        self.write(self.original['path'], b'new current source, not historical\n')
        self.write(self.python['path'], b'new current Python, not historical\n')
        self.save_manifest()
        self.record_binding = patch.object(verifier, 'SOURCE_RECORD_PIN', self.record_pin)
        self.record_binding.start()
        self.addCleanup(self.record_binding.stop)

    @staticmethod
    def json_bytes(value):
        return (json.dumps(value, sort_keys=True) + '\n').encode()

    @staticmethod
    def entry(original):
        witness = dict(original)
        witness['path'] = f"{verifier.SOURCE_WITNESS_ROOT}/{original['path']}"
        return {'original': dict(original), 'witness': witness}

    def write(self, path, raw):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(raw)
        return target

    def save_manifest(self):
        self.write(verifier.SOURCE_WITNESS_MANIFEST, self.json_bytes(self.manifest))

    def load(self):
        return verifier.SourceWitnesses(self.store, verifier.DEFAULT_RECORD, self.record)

    def pins(self, witnesses):
        captured = pin('target/debug/consumer', b'binary')
        retained = pin('retained/consumer', b'binary')
        self.write(retained['path'], b'binary')
        return verifier.Pins(self.store, captured, retained, witnesses)

    def test_exact_witnesses_replace_changed_current_sources_as_data(self):
        witnesses = self.load()
        selected = self.pins(witnesses)
        self.assertEqual(selected.check(self.original).read_bytes(), b'historical Rust source\n')
        self.assertEqual(selected.check(self.python).read_bytes(),
                         b"raise RuntimeError('historical witness must only be read')\n")
        self.assertNotEqual(self.store.pin(self.original['path']), self.original)
        self.assertIsNone(witnesses.select(pin('unmapped/data', b'data')))
        witnesses.revalidate()

    def test_changed_requested_original_pin_never_selects_an_old_witness(self):
        witnesses = self.load()
        changed = dict(self.original)
        changed['sha256'] = '0' * 64
        with self.assertRaises(AssertionError):
            self.pins(witnesses).check(changed)

    def test_tampered_witness_fails_even_when_current_source_matches_original(self):
        entry = self.manifest['source_witnesses'][0]
        self.write(self.original['path'], b'historical Rust source\n')
        self.write(entry['witness']['path'], b'wrong witness\n')
        with self.assertRaises(AssertionError):
            self.load()

    def test_correlated_witness_and_manifest_digest_change_cannot_redefine_original(self):
        entry = self.manifest['source_witnesses'][0]
        forged = b'correlated alternate source\n'
        self.write(entry['witness']['path'], forged)
        entry['witness'] = pin(entry['witness']['path'], forged)
        self.save_manifest()
        with self.assertRaises(AssertionError):
            self.load()

    def test_duplicate_unknown_and_missing_source_mappings_refuse(self):
        original = copy.deepcopy(self.manifest)
        for mutation in ['duplicate', 'unknown', 'missing']:
            with self.subTest(mutation=mutation):
                self.manifest = copy.deepcopy(original)
                entries = self.manifest['source_witnesses']
                if mutation == 'duplicate':
                    entries.append(copy.deepcopy(entries[0]))
                elif mutation == 'unknown':
                    entries.append(self.entry(pin('unrecorded/source.rs', b'new')))
                else:
                    entries.pop()
                self.save_manifest()
                with self.assertRaises(AssertionError):
                    self.load()

    def test_absolute_parent_and_wrong_namespace_paths_refuse(self):
        original = copy.deepcopy(self.manifest)
        for wrong in ['/outside/source.rs', '../source.rs', 'retained/source.rs']:
            with self.subTest(path=wrong):
                self.manifest = copy.deepcopy(original)
                self.manifest['source_witnesses'][0]['witness']['path'] = wrong
                self.save_manifest()
                with self.assertRaises((AssertionError, ValueError)):
                    self.load()

    def test_witness_symlink_cannot_leave_its_dedicated_namespace(self):
        witness = self.root / self.manifest['source_witnesses'][0]['witness']['path']
        other = self.write('elsewhere/source.rs', b'historical Rust source\n')
        witness.unlink()
        try:
            witness.symlink_to(other)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f'symlinks unavailable: {type(error).__name__}')
        with self.assertRaises((AssertionError, ValueError)):
            self.load()

    def test_manifest_cannot_rebind_original_record(self):
        changed = dict(self.record_pin)
        changed['sha256'] = 'f' * 64
        self.manifest['record'] = changed
        self.save_manifest()
        with self.assertRaises(AssertionError):
            self.load()

    def test_supplied_record_facts_cannot_override_the_anchored_file(self):
        self.record = {'source_files': []}
        self.manifest['source_witnesses'] = []
        self.save_manifest()
        with self.assertRaises(AssertionError):
            self.load()

    def test_original_record_bytes_must_match_the_fixed_record_binding(self):
        self.write(verifier.DEFAULT_RECORD, self.json_bytes({'source_files': []}))
        with self.assertRaises(AssertionError):
            self.load()

    def test_each_selection_and_final_revalidation_recheck_saved_bytes(self):
        witnesses = self.load()
        selected = self.pins(witnesses)
        selected.check(self.original)
        self.write(self.manifest['source_witnesses'][0]['witness']['path'], b'changed mid-run')
        with self.assertRaises(AssertionError):
            selected.check(self.original)
        with self.assertRaises(AssertionError):
            witnesses.revalidate()

    def test_manifest_changes_after_binding_refuse_at_final_revalidation(self):
        witnesses = self.load()
        self.manifest['extra_untrusted_note'] = 'changed after loading'
        self.save_manifest()
        with self.assertRaises(AssertionError):
            witnesses.revalidate()

    def test_cached_mapping_identity_is_rechecked_at_selection(self):
        witnesses = self.load()
        selected = self.pins(witnesses)
        witnesses.entries[self.original['path']]['witness']['sha256'] = '0' * 64
        with self.assertRaises(AssertionError):
            selected.check(self.original)

    def test_captured_consumer_substitution_stays_exact_and_separate(self):
        witnesses = self.load()
        selected = self.pins(witnesses)
        self.assertEqual(selected.check(selected.captured).read_bytes(), b'binary')
        changed = dict(selected.captured)
        changed['sha256'] = '0' * 64
        with self.assertRaises(AssertionError):
            selected.check(changed)


if __name__ == '__main__':
    unittest.main()
