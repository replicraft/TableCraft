"""Synthetic license texts test policy enforcement, not real license selection."""
import copy
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import urllib.request
from unittest.mock import patch

PACKAGING = Path(__file__).resolve().parents[1] / "packaging"
sys.path.insert(0, str(PACKAGING))
import desktop_notices as notices
import release_contract as contract


class DesktopNoticesTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="tablecraft-notice-policy-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion="0.1.0"\nlicense="Apache-2.0"\n')
        (self.root / "Cargo.lock").write_text('''version=4
[[package]]
name="alpha"
version="1.0.0"
source="registry+https://example.invalid/index"
checksum="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
[[package]]
name="beta"
version="2.0.0"
source="registry+https://example.invalid/index"
checksum="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
''')
        self.metadata = {"packages": [], "workspace_members": ["desktop"]}
        for name, version, identifier, license in [
            ("tablecraft-desktop", "0.1.0", "desktop", "Apache-2.0"),
            ("alpha", "1.0.0", "alpha", "MIT OR Apache-2.0"),
            ("beta", "2.0.0", "beta", "MIT AND BSD-2-Clause"),
        ]:
            directory = self.root / name
            directory.mkdir()
            self.metadata["packages"].append({
                "name": name, "version": version, "id": identifier,
                "license": license,
                "source": None if name == "tablecraft-desktop" else "registry+https://example.invalid/index",
                "manifest_path": str(directory / "Cargo.toml"),
            })
        self.trees = {
            target: "tablecraft-desktop v0.1.0 (/private/checkout)|\n"
                    "alpha v1.0.0|a,z\nbeta v2.0.0|default\nalpha v1.0.0|a,z (*)\n"
            for target in contract.TARGETS
        }
        self.inventory, self.locations = notices.collect(self.root, self.metadata, self.trees)
        self.policy = {
            "schema": 1, "review_status": "reviewed",
            "reviewed_inventory_sha256": notices.fingerprint(self.inventory),
            "packages": [],
            "assets": [],
        }
        self.upstream = {"schema": 1, "review_status": "reviewed", "packages": []}
        shared = b"Complete synthetic license.\r\nCopyright Example A.\r\n"
        for package in self.inventory["packages"]:
            name = package["name"]
            if package["source"] == "workspace":
                continue
            base = self.locations[(name, package["version"])]
            source = self.source(base, "LICENSE.txt", shared, "license")
            entry = {
                "name": name, "version": package["version"],
                "declared_license": package["declared_license"],
                "selected_license": package["declared_license"],
                "review_status": "reviewed", "sources": [source],
            }
            if name == "beta":
                entry["additional_obligations"] = ["Complete independent additional BSD notice"]
                entry["sources"].append(self.source(
                    base, "NOTICE.txt", b"Complete independent additional BSD obligations.\n", "notice",
                ))
            self.policy["packages"].append(entry)
        data = b"Original project license, separately shipped.\n"
        self.source(self.root, "LICENSE", data, "license")
        self.policy["project_license"] = {
            "declared_license": "Apache-2.0", "path": "LICENSE", "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
        }
        self.policy["project_packages"] = [{
            "name": "tablecraft-desktop", "version": "0.1.0", "declared_license": "Apache-2.0",
        }]
        binary = b"Independent compiled data bytes"
        binding = self.source(self.root, "font.bin", binary, "attribution")
        binding.pop("role")
        binding["root"] = "repository"
        self.policy["content_bindings"] = [binding]
        self.policy["assets"].append({
            "name": "Synthetic compiled font", "selected_license": "OFL-1.1",
            "review_status": "reviewed",
            "sources": [self.source(self.root, "font-notice.txt", b"Complete font notice.\n", "attribution")],
        })

    def source(self, root, path, data, role):
        (root / path).write_bytes(data)
        return {"path": path, "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data), "role": role}

    def assemble(self):
        return notices.assemble(self.root, self.inventory, self.locations, self.policy,
                                self.upstream, self.root / "cache")

    def test_inventory_is_stable_private_path_free_and_feature_bound(self):
        text = json.dumps(self.inventory)
        self.assertNotIn(str(self.root), text)
        self.assertNotIn("/private/checkout", text)
        self.assertEqual(len(self.inventory["targets"]), 6)
        self.assertEqual(self.inventory["targets"][next(iter(self.trees))][0]["features"], ["a", "z"])
        changed = copy.deepcopy(self.trees)
        target = next(iter(changed))
        changed[target] = changed[target].replace("a,z", "a,z,new")
        inventory, _ = notices.collect(self.root, self.metadata, changed)
        self.assertNotEqual(notices.fingerprint(inventory), notices.fingerprint(self.inventory))
        with self.assertRaisesRegex(ValueError, "inventory changed"):
            notices.assemble(self.root, inventory, self.locations, self.policy, self.upstream, self.root)

    def compact_policy(self):
        policy = {
            "schema": 1,
            "license_choices": {"MIT OR Apache-2.0": "MIT"},
            "packages": [copy.deepcopy(self.policy["packages"][1])],
            "assets": copy.deepcopy(self.policy["assets"]),
            "content_bindings": copy.deepcopy(self.policy["content_bindings"]),
        }
        alpha = self.locations[("alpha", "1.0.0")]
        (alpha / "LICENSE-MIT").write_bytes(b"Exact original MIT grant.\n")
        (alpha / "nested").mkdir()
        (alpha / "nested/NOTICE").write_bytes(b"Nested attribution.\n")
        return policy

    def test_compact_policy_generates_local_sources_hashes_and_project_authority(self):
        policy = self.compact_policy()
        expanded = notices.expand_policy(self.root, self.inventory, self.locations, policy)
        self.assertEqual(expanded["review_status"], "generated")
        alpha = expanded["packages"][0]
        paths = {source["path"] for source in alpha["sources"]}
        self.assertIn("LICENSE-MIT", paths)
        self.assertIn("nested/NOTICE", paths)
        self.assertEqual(expanded["project_packages"], self.policy["project_packages"])
        self.assertEqual(expanded["project_license"], self.policy["project_license"])
        output = notices.assemble(self.root, self.inventory, self.locations, expanded,
                                  {"schema": 1, "review_status": "generated", "packages": []}, self.root)
        self.assertIn(b"Nested attribution.", output)
        self.assertNotIn(b"Reviewed inventory", output)
        again = notices.expand_policy(self.root, self.inventory, self.locations, policy)
        self.assertEqual(expanded, again)
        self.assertEqual(policy["packages"][0], self.policy["packages"][1])

    def test_discovery_refuses_unknown_choices_missing_grants_and_unsafe_sources(self):
        policy = self.compact_policy()
        policy["license_choices"] = {}
        with self.assertRaisesRegex(ValueError, "explicit choice"):
            notices.expand_policy(self.root, self.inventory, self.locations, policy)
        base = self.locations[("alpha", "1.0.0")]
        (base / "LICENSE-MIT").unlink()
        # A generic LICENSE does not establish the selected MIT alternative.
        with self.assertRaisesRegex(ValueError, "explicit source exception"):
            notices.published_sources(base, "MIT", "MIT OR Apache-2.0")
        (base / "LICENSE-MIT").symlink_to(self.root / "LICENSE")
        with self.assertRaises(ValueError):
            notices.published_sources(base, "MIT", "MIT OR Apache-2.0")

    def test_generated_bundle_is_temporary_and_explicit_input_is_preserved(self):
        with patch.object(notices, "generate", return_value=({}, {}, b"Generated exact notices.\n")) as generate:
            with notices.bundle() as path:
                self.assertEqual(path.read_bytes(), b"Generated exact notices.\n")
                temporary = path
            self.assertFalse(temporary.exists())
            self.assertEqual(generate.call_count, 1)
            explicit = self.root / "LICENSE"
            with notices.bundle(explicit) as path:
                self.assertEqual(path, explicit)
            self.assertTrue(explicit.exists())
            self.assertEqual(generate.call_count, 1)

    def test_inventory_rejects_ambiguous_sources_and_featureless_or_missing_targets(self):
        metadata = copy.deepcopy(self.metadata)
        metadata["packages"].append(dict(metadata["packages"][1], id="other source"))
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            notices.collect(self.root, metadata, self.trees)
        changed = dict(self.trees)
        changed[next(iter(changed))] = "alpha v1.0.0\\tMIT"
        with self.assertRaisesRegex(ValueError, "Feature-bearing"):
            notices.collect(self.root, self.metadata, changed)
        changed.pop(next(iter(changed)))
        with self.assertRaisesRegex(ValueError, "six native"):
            notices.collect(self.root, self.metadata, changed)

    def test_identical_complete_bytes_deduplicate_but_additional_obligations_remain(self):
        result = self.assemble()
        self.assertEqual(result.count(b"Complete synthetic license.\r\nCopyright Example A.\r\n"), 1)
        self.assertIn(b"Complete independent additional BSD obligations.", result)
        self.assertIn(b"MIT AND BSD-2-Clause", result)
        self.assertIn(b"additional obligations: Complete independent additional BSD notice", result)
        self.assertIn(b"Complete font notice.", result)
        self.assertIn(b"beta 2.0.0", result)
        self.assertNotIn(b"tablecraft-desktop 0.1.0 |", result)
        self.assertNotIn(b"Original project license, separately shipped.", result)
        self.assertNotIn(str(self.root).encode(), result)

    def test_pending_incomplete_duplicate_and_changed_license_policies_refuse(self):
        original = copy.deepcopy(self.policy)
        for alter in [
            lambda: self.policy.update(review_status="candidate"),
            lambda: self.policy["packages"][0].update(review_status="pending"),
            lambda: self.policy["packages"].pop(),
            lambda: self.policy["packages"].append(copy.deepcopy(self.policy["packages"][0])),
            lambda: self.policy["packages"][0].update(declared_license="different"),
            lambda: self.policy.update(assets=[]),
            lambda: self.policy["packages"][0].update(selected_license=""),
        ]:
            self.policy = copy.deepcopy(original)
            alter()
            with self.assertRaises(ValueError):
                self.assemble()

    def test_changed_text_invalid_roles_and_path_escape_refuse(self):
        original = copy.deepcopy(self.policy)
        for field, value in [("sha256", "0" * 64), ("bytes", 1), ("role", "ignored"),
                             ("path", "../font-notice.txt"), ("path", "/tmp/arbitrary")]:
            self.policy = copy.deepcopy(original)
            self.policy["packages"][0]["sources"][0][field] = value
            with self.assertRaises(ValueError):
                self.assemble()

    def test_reviewed_upstream_sources_use_explicit_cache_and_preserve_source_text(self):
        entry = self.policy["packages"].pop(0)
        cache = self.root / "cache"
        cache.mkdir()
        data = b"Pinned upstream copyright and full obligations.\n"
        source = self.source(cache, "original.txt", data, "license")
        source["cache_file"] = source.pop("path")
        source["url"] = "https://example.invalid/repository/pinned-revision/LICENSE"
        entry["sources"] = [source]
        self.upstream["packages"].append(entry)
        self.assertIn(data, self.assemble())
        source["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            self.assemble()

    def test_project_authority_and_compiled_content_cannot_change_silently(self):
        original = copy.deepcopy(self.policy)
        for alter in [
            lambda: self.policy.update(project_packages=[]),
            lambda: self.policy["project_license"].update(sha256="0" * 64),
            lambda: self.policy["content_bindings"][0].update(sha256="0" * 64),
            lambda: self.policy.update(content_bindings=[]),
        ]:
            self.policy = copy.deepcopy(original)
            alter()
            with self.assertRaises(ValueError):
                self.assemble()

    def test_derived_font_notice_requires_unchanged_parent_binary(self):
        source = self.policy["assets"][0]["sources"][0]
        source["derived_from"] = dict(self.policy["content_bindings"][0])
        self.assemble()
        (self.root / "font.bin").write_bytes(b"Changed font binary")
        # First allow the content inventory change, then prove the extracted
        # attribution still independently binds its original parent binary.
        changed = (self.root / "font.bin").read_bytes()
        self.policy["content_bindings"][0].update(
            bytes=len(changed), sha256=hashlib.sha256(changed).hexdigest(),
        )
        with self.assertRaisesRegex(ValueError, "parent binary changed"):
            self.assemble()

    def test_reviewed_source_header_extraction_keeps_exact_lines_and_full_input_binding(self):
        entry = self.policy["packages"][0]
        base = self.locations[(entry["name"], entry["version"])]
        full = b"// exact copyright\r\n// exact grant reference\r\nfn unrelated_code() {}\r\n"
        source = self.source(base, "original.rs", full, "attribution")
        source["extraction"] = {"start_line": 1, "end_line": 2}
        entry["sources"].append(source)
        output = self.assemble()
        self.assertIn(b"// exact copyright\r\n// exact grant reference\r\n", output)
        self.assertNotIn(b"fn unrelated_code", output)
        source["role"] = "license"
        with self.assertRaisesRegex(ValueError, "grants remain complete"):
            self.assemble()
        source["role"] = "attribution"
        source["extraction"]["end_line"] = 9
        with self.assertRaisesRegex(ValueError, "range is outside"):
            self.assemble()
        source["extraction"]["end_line"] = 2
        (base / "original.rs").write_bytes(full + b"changed unextracted bytes")
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            self.assemble()

    def test_explicit_fetch_is_hash_checked_reviewed_and_never_executes_content(self):
        data = b"Pinned complete license source bytes.\n"
        source = {
            "cache_file": "licenses/original.txt", "url": "https://example.invalid/pin/LICENSE",
            "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(), "role": "license",
        }
        entry = {
            "review_status": "reviewed", "selected_license": "MIT", "sources": [source],
        }
        document = {"schema": 1, "review_status": "reviewed", "packages": [entry, copy.deepcopy(entry)]}
        response = io.BytesIO(data)
        response.geturl = lambda: source["url"]
        opener = unittest.mock.Mock()
        opener.open.return_value = response
        cache = self.root / "fetched"
        with patch.object(notices.subprocess, "run", side_effect=AssertionError("No execution")):
            notices.fetch_sources([document], cache, opener)
            self.assertEqual((cache / "licenses/original.txt").read_bytes(), data)
            notices.fetch_sources([document], cache, opener)
        self.assertEqual(opener.open.call_count, 1)
        self.assertEqual(opener.open.call_args.kwargs["timeout"], 30)

        changed = copy.deepcopy(document)
        changed["packages"][1]["sources"][0]["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "Conflicting"):
            notices.fetch_sources([changed], self.root / "conflict", opener)
        entry["review_status"] = "candidate"
        with self.assertRaisesRegex(ValueError, "explicit review"):
            notices.fetch_sources([document], self.root / "pending", opener)

    def test_fetch_refuses_unsafe_urls_redirects_and_wrong_bytes_before_creation(self):
        for url in ["http://example.invalid/LICENSE", "https://user:secret@example.invalid/LICENSE",
                    "https://example.invalid/LICENSE#fragment"]:
            with self.assertRaises(ValueError):
                notices.https_url(url)
        handler = notices.SameOriginRedirect()
        request = urllib.request.Request("https://example.invalid/pin/LICENSE")
        for url in ["http://example.invalid/LICENSE", "https://another.invalid/LICENSE"]:
            with self.assertRaises(ValueError):
                handler.redirect_request(request, None, 302, "redirect", {}, url)
        source = {
            "cache_file": "wrong.txt", "url": "https://example.invalid/pin/LICENSE",
            "bytes": 3, "sha256": hashlib.sha256(b"abc").hexdigest(), "role": "license",
        }
        document = {"schema": 1, "review_status": "reviewed", "packages": [{
            "review_status": "reviewed", "selected_license": "MIT", "sources": [source],
        }]}
        for wrong in [b"ab", b"abcd", b"xyz"]:
            response = io.BytesIO(wrong)
            response.geturl = lambda: source["url"]
            opener = unittest.mock.Mock()
            opener.open.return_value = response
            cache = self.root / "invalid-fetch"
            with self.assertRaisesRegex(ValueError, "size/hash"):
                notices.fetch_sources([document], cache, opener)
            self.assertFalse((cache / "wrong.txt").exists())

    def test_inline_attribution_is_exact_parent_bound_and_never_used_as_a_license_grant(self):
        source = self.policy["assets"][0]["sources"][0]
        data = b"Exact embedded font copyright.\n"
        source.clear()
        source.update(
            role="attribution", text=data.decode(), bytes=len(data),
            sha256=hashlib.sha256(data).hexdigest(),
            derived_from=dict(self.policy["content_bindings"][0]),
        )
        self.assertIn(data, self.assemble())
        self.assertEqual(notices.cached_sources([self.policy]), {})
        source["role"] = "license"
        with self.assertRaisesRegex(ValueError, "only for parent-bound"):
            self.assemble()
        source["role"] = "attribution"
        source.pop("derived_from")
        with self.assertRaisesRegex(ValueError, "only for parent-bound"):
            self.assemble()

    def test_canonical_terms_do_not_impersonate_package_attribution(self):
        entry = self.policy["packages"][0]
        entry["sources"][0]["canonical"] = True
        output = self.assemble()
        self.assertIn(b"placeholder is unmodified, not package attribution", output)
        entry["sources"][0]["role"] = "attribution"
        with self.assertRaisesRegex(ValueError, "cannot stand in for attribution"):
            self.assemble()

    def test_notice_output_requires_a_fresh_path_and_preserves_existing_bytes(self):
        path = self.root / "review-output.txt"
        notices.write_new(path, b"Reviewed original")
        with self.assertRaises(FileExistsError):
            notices.write_new(path, b"Replacement")
        self.assertEqual(path.read_bytes(), b"Reviewed original")

    def test_live_inventory_commands_are_locked_offline_normal_and_exclude_proc_macros(self):
        replies = [subprocess.CompletedProcess([], 0, json.dumps(self.metadata), "")]
        replies += [subprocess.CompletedProcess([], 0, tree, "") for _, tree in sorted(self.trees.items())]
        with patch.object(notices.subprocess, "run", side_effect=replies) as run:
            inventory, _ = notices.collect(self.root)
        self.assertEqual(inventory, self.inventory)
        self.assertEqual(len(run.call_args_list), 7)
        for call in run.call_args_list:
            arguments = call.args[0]
            self.assertIn("--locked", arguments)
            self.assertIn("--offline", arguments)
            self.assertNotIn("build", arguments)
            if "tree" in arguments:
                self.assertIn("normal,no-proc-macro", arguments)
                self.assertIn("{p}|{f}", arguments)


if __name__ == "__main__":
    unittest.main()
