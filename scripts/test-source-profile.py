"""Regression checks for the published SDK and reviewed release profile."""
from pathlib import Path
import subprocess
import tomllib
import unittest
import re


ROOT = Path(__file__).resolve().parent.parent
SDK_REV = "207a9a7df15e44fdba00a807d0f7eaf21904ba0e"
TSD_TAG = "v1.6.0"
CI_PIN = "4adc1080fb495098e7af1aade82ed7e6e3c69bb7"
RELEASE_PIN = "6ed83b04aab2d8977ec62e966cb914531e534fa4"


def manifest(relative_path):
    return tomllib.loads((ROOT / relative_path).read_text())


class PublishedSdkProfile(unittest.TestCase):
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

    def test_standalone_patch_and_nested_gitlink_share_the_published_sdk(self):
        patch = manifest("Cargo.toml")["patch"]["https://github.com/tinyhumansai/tinybus"]
        self.assertEqual(patch["tinybus"]["path"], "vendor/tinybus/crates/tinybus")
        self.assertEqual(patch["tinybus-module"]["path"], "vendor/tinybus/crates/tinybus-module")
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
                self.assertTrue((ROOT / relative_path).stat().st_mode & 0o111)


if __name__ == "__main__":
    unittest.main()
