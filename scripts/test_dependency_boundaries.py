"""Regression cases for architectural violations and manifest bypasses."""
from copy import deepcopy
import unittest

from check_dependency_boundaries import violations


TESTKIT_SUPPORT_TARGETS = (
    "tablecraft-ace", "tablecraft-access", "tablecraft-query",
    "tablecraft-db", "tablecraft-model", "tablecraft-schema",
    "tablecraft-sql", "tablecraft-io",
)


def package(name, directory, dependencies=(), library=True):
    return {"name": name, "id": name, "manifest_path": f"/repo/{directory}/Cargo.toml",
            "targets": [{"kind": ["lib" if library else "bin"]}],
            "dependencies": [{"name": dep} for dep in dependencies]}


def workspace(*packages):
    return {"workspace_root": "/repo", "workspace_members": [p["id"] for p in packages],
            "packages": list(packages)}


class BoundaryTests(unittest.TestCase):
    def setUp(self):
        self.base = workspace(
            package("tablecraft-core", "crates/foundation/core"),
            package("tablecraft-schema", "crates/foundation/schema", ["tablecraft-core"]),
            package("tablecraft-db", "crates/foundation/db", ["tablecraft-core", "tablecraft-schema"]),
            package("tablecraft-ace", "crates/engines/ace", ["tablecraft-core"]),
            package("tablecraft-access", "crates/backends/access", ["tablecraft-db", "tablecraft-ace"]),
            package("tablecraft-query", "crates/query/query", ["tablecraft-db"]),
            package("tablecraft-cli", "apps/cli", ["tablecraft-access", "tablecraft-query"], False),
        )

    def test_adapter_composition_and_generic_query_contracts_pass(self):
        self.assertEqual(violations(self.base), [])

    def test_engine_cannot_import_adapter_or_generic_db(self):
        for target in ["tablecraft-access", "tablecraft-db"]:
            with self.subTest(target=target):
                data = deepcopy(self.base)
                data["packages"][3]["dependencies"].append({"name": target})
                self.assertTrue(violations(data))

    def test_query_cannot_import_engine_through_alias_optional_or_target(self):
        for extra in [{"rename": "storage"}, {"optional": True},
                      {"target": 'cfg(target_os = "windows")'}]:
            with self.subTest(extra=extra):
                data = deepcopy(self.base)
                data["packages"][5]["dependencies"].append({"name": "tablecraft-ace", **extra})
                self.assertTrue(violations(data))

    def test_libraries_cannot_import_application_in_any_dependency_kind(self):
        for kind in [None, "dev", "build"]:
            with self.subTest(kind=kind):
                data = deepcopy(self.base)
                data["packages"][3]["dependencies"].append({"name": "tablecraft-cli", "kind": kind})
                self.assertTrue(any("application" in e for e in violations(data)))

    def test_path_dependency_cannot_bypass_workspace_review(self):
        data = deepcopy(self.base)
        data["packages"][3]["dependencies"].append({"name": "hidden-host", "path": "/external/host"})
        self.assertTrue(any("outside reviewed" in e for e in violations(data)))

    def test_path_identity_wins_over_a_disguised_dependency_name(self):
        data = deepcopy(self.base)
        data["packages"][3]["dependencies"].append({"name": "tablecraft-core", "path": "/repo/apps/cli"})
        self.assertTrue(any("application" in e for e in violations(data)))

    def test_new_packages_require_review_even_without_dependencies(self):
        data = deepcopy(self.base)
        new = package("tablecraft-python", "crates/application/python")
        data["packages"].append(new)
        data["workspace_members"].append(new["id"])
        self.assertTrue(any("no reviewed" in e for e in violations(data)))

    def test_registry_and_engineering_tools_are_not_local_package_edges(self):
        data = deepcopy(self.base)
        data["packages"][3]["dependencies"].append({"name": "proptest", "kind": "dev"})
        self.assertEqual(violations(data), [])

    def test_shared_fixtures_are_development_only(self):
        for kind in [None, "build", "dev"]:
            with self.subTest(kind=kind):
                data = deepcopy(self.base)
                helper = package("tablecraft-testkit", "crates/engineering/tablecraft-testkit")
                data["packages"].append(helper)
                data["workspace_members"].append(helper["id"])
                data["packages"][3]["dependencies"].append({
                    "name": "tablecraft-testkit", "kind": kind,
                    "optional": True, "rename": "fixtures",
                })
                errors = violations(data)
                if kind == "dev":
                    self.assertEqual(errors, [])
                else:
                    self.assertTrue(any("development-only" in error for error in errors))

    def test_testkit_core_values_pass_but_nonoptional_engine_support_is_rejected(self):
        data = deepcopy(self.base)
        helper = package("tablecraft-testkit", "crates/engineering/tablecraft-testkit",
                         ["tablecraft-core"])
        data["packages"].append(helper)
        data["workspace_members"].append(helper["id"])
        self.assertEqual(violations(data), [])

        helper["dependencies"].append({"name": "tablecraft-ace"})
        self.assertTrue(violations(data))

    def _testkit_workspace(self):
        data = deepcopy(self.base)
        for name, directory in [
            ("tablecraft-model", "crates/foundation/model"),
            ("tablecraft-io", "crates/platform/io"),
            ("tablecraft-sql", "crates/query/sql"),
            ("tablecraft-crypto", "crates/platform/crypto"),
        ]:
            support = package(name, directory)
            data["packages"].append(support)
            data["workspace_members"].append(support["id"])
        helper = package(
            "tablecraft-testkit",
            "crates/engineering/tablecraft-testkit",
            ["tablecraft-core"],
        )
        data["packages"].append(helper)
        data["workspace_members"].append(helper["id"])
        return data, helper

    def test_reviewed_testkit_support_accepts_only_optional_normal_edges(self):
        for target in TESTKIT_SUPPORT_TARGETS:
            for extra in [
                {},
                {"rename": "domain_support", "target": 'cfg(target_os = "windows")'},
            ]:
                with self.subTest(target=target, extra=extra):
                    data, helper = self._testkit_workspace()
                    helper["dependencies"].append({
                        "name": target,
                        "kind": None,
                        "optional": True,
                        **extra,
                    })
                    self.assertEqual(violations(data), [])

    def test_reviewed_testkit_support_rejects_nonoptional_normal_edges(self):
        for target in TESTKIT_SUPPORT_TARGETS:
            for extra in [{}, {"optional": False}]:
                with self.subTest(target=target, extra=extra):
                    data, helper = self._testkit_workspace()
                    helper["dependencies"].append({"name": target, **extra})
                    self.assertTrue(any(
                        f"tablecraft-testkit -> {target}" in error
                        for error in violations(data)
                    ))

    def test_reviewed_testkit_support_rejects_build_and_dev_edges(self):
        for target in TESTKIT_SUPPORT_TARGETS:
            for kind in ["build", "dev"]:
                with self.subTest(target=target, kind=kind):
                    data, helper = self._testkit_workspace()
                    helper["dependencies"].append({
                        "name": target,
                        "kind": kind,
                        "optional": True,
                        "rename": "domain_support",
                        "target": 'cfg(target_os = "windows")',
                    })
                    self.assertTrue(any(
                        f"tablecraft-testkit -> {target}" in error
                        for error in violations(data)
                    ))

    def test_optional_support_does_not_admit_other_packages_or_apps(self):
        for target in ["tablecraft-crypto", "tablecraft-cli"]:
            with self.subTest(target=target):
                data, helper = self._testkit_workspace()
                helper["dependencies"].append({
                    "name": target,
                    "optional": True,
                    "rename": "domain_support",
                    "target": 'cfg(target_os = "windows")',
                })
                errors = violations(data)
                self.assertTrue(any(
                    f"tablecraft-testkit -> {target}" in error
                    for error in errors
                ))
                if target == "tablecraft-cli":
                    self.assertTrue(any("application" in error for error in errors))

    def test_query_testkit_incoming_edges_remain_dev_only_even_when_disguised(self):
        for kind in [None, "build", "dev"]:
            for optional in [False, True]:
                for identity in [
                    {"name": "tablecraft-testkit", "rename": "fixtures"},
                    {"name": "tablecraft-core", "path": "/repo/crates/engineering/tablecraft-testkit"},
                ]:
                    with self.subTest(kind=kind, optional=optional, identity=identity):
                        data, _ = self._testkit_workspace()
                        query = next(
                            candidate for candidate in data["packages"]
                            if candidate["name"] == "tablecraft-query"
                        )
                        query["dependencies"].append({
                            **identity,
                            "kind": kind,
                            "optional": optional,
                            "target": 'cfg(target_os = "windows")',
                        })
                        errors = violations(data)
                        if kind == "dev":
                            self.assertEqual(errors, [])
                        else:
                            self.assertTrue(any(
                                "tablecraft-query -> tablecraft-testkit" in error
                                and "development-only" in error
                                for error in errors
                            ))

    def test_library_placement_is_checked(self):
        data = deepcopy(self.base)
        data["packages"][0]["manifest_path"] = "/repo/apps/core/Cargo.toml"
        self.assertTrue(any("belongs under crates" in e for e in violations(data)))

    def test_ffi_and_macro_libraries_have_the_same_host_boundary(self):
        for kind in ["rlib", "dylib", "cdylib", "staticlib", "proc-macro"]:
            with self.subTest(kind=kind):
                data = deepcopy(self.base)
                data["packages"][3]["targets"] = [{"kind": [kind]}]
                data["packages"][3]["dependencies"].append({"name": "tablecraft-cli"})
                self.assertTrue(any("application" in e for e in violations(data)))


if __name__ == "__main__":
    unittest.main()
