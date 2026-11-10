"""Authenticated tool/cache/source preparation; no native build is simulated."""
import hashlib
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import urllib.request

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import linux_tools as tools
from test_package_cli import native_binary

TARGET = "aarch64-unknown-linux-gnu"


def identity(data):
    return {"size": len(data), "sha256": hashlib.sha256(data).hexdigest()}


class Response(io.BytesIO):
    status = 200

    def __init__(self, data):
        super().__init__(data)
        self.headers = {"Content-Length": str(len(data))}


class Network:
    def __init__(self, payloads):
        self.payloads = payloads
        self.requests = []

    def open(self, request, timeout):
        self.requests.append(request)
        return Response(self.payloads[request.full_url])


class LinuxToolsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.output = self.root / "tools"
        self.inventory = self.root / "inventory.json"
        self.metadata = json.loads(tools.INVENTORY.read_bytes())
        self.payloads = {}
        for item in self.metadata["sources"]:
            notices = [entry for entry in self.metadata["licenses"] if entry["source"] == item["path"]]
            if notices:
                stream = io.BytesIO()
                with tarfile.open(fileobj=stream, mode="w:gz") as archive:
                    for entry in notices:
                        for name in entry["members"]:
                            data = ("Synthetic source notice for " + name + "\n").encode()
                            member = tarfile.TarInfo(name)
                            member.size = len(data)
                            archive.addfile(member, io.BytesIO(data))
                data = stream.getvalue()
            else:
                data = ("Synthetic source/patch/license " + item["path"] + "\n").encode()
            item.update(identity(data))
            if "sha512" in item:
                item["sha512"] = hashlib.sha512(data).hexdigest()
            self.payloads[item["url"]] = data
        for target, assets in self.metadata["targets"].items():
            for name, item in assets.items():
                data = bytearray(native_binary(target))
                data[8:11] = b"AI\x02"
                data.extend(name.encode())
                data = bytes(data)
                item.update(identity(data))
                self.payloads[item["url"]] = data
        self.save_inventory()
        self.network = Network(self.payloads)

    def save_inventory(self):
        self.inventory.write_text(json.dumps(self.metadata))

    def fetch(self):
        return tools.fetch(TARGET, self.output, inventory=self.inventory, opener=self.network)

    def rewrite_receipt(self, name):
        path = self.output / "receipt.json"
        receipt = json.loads(path.read_bytes())
        receipt["files"][name] = tools.contract.digest(self.output / name)
        path.write_text(json.dumps(receipt))

    def test_complete_kit_and_offline_cache_reuse(self):
        receipt = self.fetch()
        self.assertEqual(set(receipt["files"]), {
            "linuxdeploy", "appimagetool", "runtime", "RuntimeThirdPartyLicenses.txt", "RuntimeSource.tar.gz",
        })
        self.assertIn("LGPL-2.1", (self.output / "RuntimeThirdPartyLicenses.txt").read_text())
        with tarfile.open(self.output / "RuntimeSource.tar.gz") as archive:
            members = archive.getmembers()
            self.assertTrue(all(member.isfile() for member in members))
            guide = archive.extractfile("RuntimeSource/README.md").read().decode()
            self.assertIn("--runtime-file ./runtime-rebuilt", guide)
            container = archive.extractfile("RuntimeSource/Containerfile").read().decode()
            self.assertIn("alpine:3.21@sha256:", container)
            self.assertNotIn("musl-dev=", container)
            self.assertNotIn("zlib-static=", container)
            rebuild = archive.extractfile("RuntimeSource/rebuild-runtime.sh").read().decode()
            self.assertIn("--prefix=/usr --disable-shared", rebuild)
            self.assertIn("-DMI_SECURE=ON", rebuild)
            self.assertIn("CVE-2026-85091.patch", rebuild)
            self.assertIn("install-static install-includes install-pc", rebuild)
        self.network.open = unittest.mock.Mock(side_effect=AssertionError("Offline cache made a request"))
        self.assertEqual(self.fetch(), receipt)

    def test_corrupt_network_payload_refuses_without_publishing(self):
        item = self.metadata["targets"][TARGET]["runtime"]
        self.payloads[item["url"]] = b"x" * item["size"]
        sentinel = self.root / "foreign"
        sentinel.write_text("keep")
        with self.assertRaisesRegex(ValueError, "digest"):
            self.fetch()
        self.assertFalse(self.output.exists())
        self.assertEqual(sentinel.read_text(), "keep")
        self.assertFalse(list(self.root.glob(".tablecraft-linux-tools-*")))

    def test_receipt_cannot_authorize_changed_upstream_binary(self):
        self.fetch()
        runtime = self.output / "runtime"
        runtime.write_bytes(b"x" * runtime.stat().st_size)
        self.rewrite_receipt("runtime")
        with self.assertRaisesRegex(ValueError, "upstream digest"):
            self.fetch()

    def test_receipt_cannot_authorize_changed_source_kit(self):
        self.fetch()
        path = self.output / "RuntimeSource.tar.gz"
        replacement = self.root / "replacement.tar.gz"
        with tarfile.open(path) as source, tarfile.open(replacement, "w:gz") as destination:
            for member in source:
                data = source.extractfile(member).read()
                if member.name == "RuntimeSource/README.md":
                    data = data.replace(b"AppImage", b"BadImage", 1)
                member.size = len(data)
                destination.addfile(member, io.BytesIO(data))
        path.write_bytes(replacement.read_bytes())
        self.rewrite_receipt("RuntimeSource.tar.gz")
        with self.assertRaisesRegex(ValueError, "canonical"):
            self.fetch()

    def test_unknown_or_link_outer_source_members_refuse(self):
        self.fetch()
        path = self.output / "RuntimeSource.tar.gz"
        with tarfile.open(path, "w:gz") as archive:
            member = tarfile.TarInfo("RuntimeSource/README.md")
            member.type = tarfile.SYMTYPE
            member.linkname = "/etc/passwd"
            archive.addfile(member)
        self.rewrite_receipt("RuntimeSource.tar.gz")
        with self.assertRaisesRegex(ValueError, "unexpected"):
            self.fetch()

    def test_unsafe_and_duplicate_source_paths_refuse_before_network(self):
        for name in ("../escape", "sources/../../escape", "sources\\escape", "/absolute"):
            with self.subTest(name=name):
                self.metadata["sources"][0]["path"] = name
                self.save_inventory()
                with self.assertRaisesRegex(ValueError, "unsafe"):
                    self.fetch()
        self.metadata["sources"][0]["path"] = self.metadata["sources"][1]["path"]
        self.save_inventory()
        with self.assertRaisesRegex(ValueError, "Duplicate"):
            self.fetch()
        self.assertEqual(self.network.requests, [])

    def test_stream_length_and_secondary_digest_are_checked(self):
        destination = self.root / "download"
        for data in (b"ab", b"abcd"):
            with self.subTest(data=data):
                with self.assertRaises(ValueError):
                    tools.store_checked(io.BytesIO(data), destination, identity(b"abc"))
                self.assertFalse(destination.exists())
        item = identity(b"abc") | {"sha512": "0" * 128}
        with self.assertRaisesRegex(ValueError, "SHA-512"):
            tools.store_checked(io.BytesIO(b"abc"), destination, item)
        self.assertFalse(destination.exists())

    def test_existing_download_is_never_deleted(self):
        destination = self.root / "download"
        destination.write_bytes(b"foreign")
        with self.assertRaises(FileExistsError):
            tools.store_checked(io.BytesIO(b"abc"), destination, identity(b"abc"))
        self.assertEqual(destination.read_bytes(), b"foreign")

    def test_redirect_strips_api_token_and_rejects_untrusted_origins(self):
        request = urllib.request.Request("https://api.github.com/repos/example/assets/1", headers={"Authorization": "Bearer private"})
        handler = tools.Redirects()
        redirected = handler.redirect_request(request, None, 302, "Found", {}, "https://release-assets.githubusercontent.com/example")
        self.assertIsNone(redirected.get_header("Authorization"))
        for url in ("http://github.com/file", "https://evil.example/file", "https://user:pass@github.com/file", "https://github.com:444/file", "https://github.com/file#fragment"):
            with self.subTest(url=url):
                with self.assertRaisesRegex(ValueError, "unauthorized"):
                    handler.redirect_request(request, None, 302, "Found", {}, url)

    def test_native_relink_refuses_wrong_host_before_docker(self):
        with patch.object(tools.sys, "platform", "darwin"), patch.object(tools.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "native Linux"):
                tools.relink(TARGET, self.output, self.inventory)
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
