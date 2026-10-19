"""Regression checks for the published SDK and reviewed release profile."""
from pathlib import Path
import os
import subprocess
import tempfile
import tomllib
import unittest
import re
from types import SimpleNamespace
from unittest.mock import patch


ROOT = Path(__file__).resolve().parent.parent
SDK_REV = "207a9a7df15e44fdba00a807d0f7eaf21904ba0e"
TSD_TAG = "v1.6.0"
CI_PIN = "4adc1080fb495098e7af1aade82ed7e6e3c69bb7"
RELEASE_PIN = "6ed83b04aab2d8977ec62e966cb914531e534fa4"


def manifest(relative_path):
    return tomllib.loads((ROOT / relative_path).read_text())


def git_tree_executable(relative_path):
    entry = subprocess.run(
        ["git", "ls-tree", "HEAD", "--", relative_path],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=True,
    ).stdout.strip()
    return bool(entry) and entry.split(maxsplit=1)[0] == "100755"


class PublishedSdkProfile(unittest.TestCase):
    def resolve_release(self, **overrides):
        environment = os.environ.copy()
        environment.update({
            "GITHUB_EVENT_NAME": "workflow_dispatch",
            "GITHUB_REF": "refs/heads/main",
            "RELEASE_MODE": "",
            "BUMP": "",
            "TAG": "",
            "VERSION": "",
            "REVIEWED_PR": "",
            **overrides,
        })
        with tempfile.NamedTemporaryFile() as output:
            environment["GITHUB_OUTPUT"] = output.name
            result = subprocess.run(
                ["bash", str(ROOT / "scripts/resolve-release-mode.sh")],
                cwd=ROOT,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
            output.seek(0)
            values = dict(
                line.split("=", 1)
                for line in output.read().decode().splitlines()
                if "=" in line
            )
        return result, values

    def test_release_router_finite_main_and_reviewed_input_matrix(self):
        valid = (
            ({"BUMP": "patch"}, {"mode": "main", "bump": "patch"}),
            (
                {
                    "RELEASE_MODE": "reviewed-epic",
                    "TAG": "v0.4.2",
                    "VERSION": "0.4.2",
                    "REVIEWED_PR": "16",
                },
                {"mode": "reviewed-epic", "tag": "v0.4.2", "version": "0.4.2", "reviewed_pr": "16"},
            ),
        )
        for inputs, expected in valid:
            with self.subTest(inputs=inputs):
                result, outputs = self.resolve_release(**inputs)
                self.assertEqual(result.returncode, 0, result.stderr)
                for key, value in expected.items():
                    self.assertEqual(outputs.get(key), value)

        invalid = (
            {"BUMP": ""},
            {"BUMP": "wat"},
            {"BUMP": "patch", "TAG": "v0.4.2"},
            {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.2", "VERSION": "0.4.2"},
            {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.2", "VERSION": "0.4.2", "REVIEWED_PR": "0"},
            {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.2", "VERSION": "0.4.2", "REVIEWED_PR": "016"},
            {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.3", "VERSION": "0.4.2", "REVIEWED_PR": "16"},
            {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.2", "VERSION": "0.4.2", "REVIEWED_PR": "16", "GITHUB_EVENT_NAME": "push"},
            {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.2", "VERSION": "0.4.2", "REVIEWED_PR": "16", "GITHUB_REF": "refs/heads/feature"},
            {"RELEASE_MODE": "unknown", "BUMP": "patch"},
        )
        for inputs in invalid:
            with self.subTest(invalid=inputs):
                result, _ = self.resolve_release(**inputs)
                self.assertNotEqual(result.returncode, 0)

        release = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("reviewed-module-release:", release)
        self.assertIn(
            "module-reviewed-epic-release.yml@" + RELEASE_PIN,
            release,
        )
        self.assertIn("module-release:", release)
        self.assertIn("needs.resolve.outputs.mode == 'main'", release)

    def test_git_executable_mode_survives_windows_filesystem_mode_bits(self):
        with patch.object(Path, "stat", return_value=SimpleNamespace(st_mode=0o100644)):
            self.assertTrue(git_tree_executable("scripts/build-module.sh"))

    def test_every_direct_tinybus_edge_uses_the_published_sdk(self):
        manifests = (
            "crates/tinymemes-client/Cargo.toml",
            "crates/tinymemes-module/Cargo.toml",
            "proof-workspaces/remix-markers/runner/Cargo.toml",
        )
        for relative_path in manifests:
            cargo = manifest(relative_path)
            for section in ("dependencies", "dev-dependencies"):
                for name, dependency in cargo.get(section, {}).items():
                    if name in {"tinybus", "tinybus-module"}:
                        with self.subTest(manifest=relative_path, name=name, section=section):
                            self.assertIn("git", dependency)
                            self.assertNotIn("path", dependency)
                            self.assertEqual(dependency["rev"], SDK_REV)
                            self.assertEqual(dependency["version"], ">=0.3.3, <0.4.0")

    def test_nested_gitlink_matches_the_published_sdk_revision(self):
        rev = subprocess.run(
            ["git", "-C", str(ROOT / "vendor/tinybus"), "rev-parse", "HEAD"],
            text=True,
            capture_output=True,
            check=True,
        ).stdout.strip()
        self.assertEqual(rev, SDK_REV)

    def test_storage_client_keeps_an_explicit_compatible_sdk_edge(self):
        dependency = manifest("crates/tinymemes-module/Cargo.toml")["dependencies"][
            "tinystoragedrivers-bus"
        ]
        self.assertEqual(dependency["tag"], TSD_TAG)
        self.assertFalse(dependency.get("default-features", True))
        self.assertIn("client", dependency.get("features", []))

    def test_storage_driver_references_move_together(self):
        manifests = (
            "crates/tinymemes-bus/Cargo.toml",
            "crates/tinymemes-module/Cargo.toml",
            "proof-workspaces/remix-markers/runner/Cargo.toml",
        )
        for relative_path in manifests:
            cargo = manifest(relative_path)
            for section in ("dependencies", "dev-dependencies"):
                for name, dependency in cargo.get(section, {}).items():
                    if name.startswith("tinystoragedrivers-"):
                        with self.subTest(manifest=relative_path, name=name, section=section):
                            self.assertEqual(dependency.get("tag"), TSD_TAG)

    def test_contract_storage_schema_stays_opt_in_and_ports_only(self):
        cargo = manifest("crates/tinymemes-bus/Cargo.toml")
        self.assertEqual(cargo["features"]["default"], [])
        self.assertIn("storage", cargo["features"])
        dependency = cargo["dependencies"]["tinystoragedrivers-core"]
        self.assertEqual(dependency["tag"], TSD_TAG)
        self.assertFalse(dependency.get("default-features", True))
        self.assertTrue(dependency["optional"])

    def test_all_released_packages_move_together(self):
        for relative_path in (
            "Cargo.toml",
            "crates/tinymemes-bus/Cargo.toml",
            "crates/tinymemes-client/Cargo.toml",
            "crates/tinymemes-module/Cargo.toml",
        ):
            with self.subTest(manifest=relative_path):
                self.assertEqual(manifest(relative_path)["package"]["version"], "0.4.2")

    def test_obsolete_tinybus_replacements_are_gone(self):
        for relative_path in ("Cargo.toml", "proof-workspaces/remix-markers/Cargo.toml"):
            with self.subTest(manifest=relative_path):
                text = (ROOT / relative_path).read_text()
                self.assertNotIn("[replace]", text)
                self.assertNotIn("f11987b0b44c92da2064a4b3787ca7527a23db26", text)

    def test_shared_module_ci_and_closed_review_release_are_pinned(self):
        ci = (ROOT / ".github/workflows/ci.yml").read_text()
        self.assertIn(f"module-ci.yml@{CI_PIN}", ci)
        delegate = (ROOT / ".github/workflows/module-reviewed-epic-release.yml").read_text()
        self.assertIn(f"module-reviewed-epic-release.yml@{RELEASE_PIN}", delegate)
        workflow_inputs = delegate.split("    inputs:\n", 1)[1].split("\npermissions:", 1)[0]
        self.assertEqual(
            re.findall(r"^      ([a-z][a-z-]*):$", workflow_inputs, re.MULTILINE),
            ["tag", "version", "reviewed-pr"],
        )
        self.assertNotIn("publish-crates-io", delegate)

    def test_ordinary_manual_release_and_new_reviewed_profile_coexist(self):
        release = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("workflow_dispatch:", release)
        self.assertIn("refs/heads/main", release)
        self.assertIn("expected 11 module archives", release)
        self.assertIn("submodules: recursive", release)
        self.assertIn("proof-workspaces/bus-contract/Cargo.lock", release)
        self.assertIn("release_mode:", release)
        self.assertIn("needs.resolve.outputs.mode == 'reviewed-epic'", release)
        self.assertIn("needs.resolve.outputs.mode == 'main'", release)
        self.assertIn("run: scripts/resolve-release-mode.sh", release)

    def test_shared_release_packagers_and_distro_runner_are_present(self):
        for relative_path in (
            "scripts/build-module.sh",
            "scripts/build-module.ps1",
            "scripts/verify-module.sh",
            "scripts/verify-module.ps1",
            "scripts/test-distro.sh",
        ):
            with self.subTest(path=relative_path):
                self.assertTrue((ROOT / relative_path).is_file())

    def test_bus_consumers_exercise_default_and_no_default_graphs(self):
        for relative_path in (
            "proof-workspaces/bus-contract/default-consumer/Cargo.toml",
            "proof-workspaces/bus-contract/no-default-consumer/Cargo.toml",
        ):
            with self.subTest(manifest=relative_path):
                self.assertTrue((ROOT / relative_path).is_file())
        self.assertIn("--features storage", (ROOT / "scripts/check-bus-consumer-graphs.sh").read_text())

    def test_release_packaging_shell_scripts_are_executable(self):
        for relative_path in (
            "scripts/build-module.sh",
            "scripts/verify-module.sh",
            "scripts/test-distro.sh",
            "scripts/install-distro-packages.sh",
        ):
            with self.subTest(path=relative_path):
                self.assertTrue(git_tree_executable(relative_path))


if __name__ == "__main__":
    unittest.main()
