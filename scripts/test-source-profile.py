"""Regression checks for the published SDK and reviewed release profile."""
from pathlib import Path
import os
import subprocess
import sys
import tempfile
import tomllib
import unittest
import re
import shutil
from types import SimpleNamespace
from unittest.mock import patch
sys.dont_write_bytecode = True
from test_support import bash_command, bash_diagnostic, git_bash_executable, run_bash


ROOT = Path(__file__).resolve().parent.parent
SDK_REV = "9e14f65666b11782ec1054aba576114026d180f8"
TSD_REV = "a04e3a174fa8f28d5229a816ca6b39c72a9efd7b"
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
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="release-router-", dir=ROOT / "target") as scratch:
            output = Path(scratch) / "github-output"
            output.touch()
            environment["GITHUB_OUTPUT"] = output.relative_to(ROOT).as_posix()
            result = run_bash(
                "scripts/resolve-release-mode.sh",
                cwd=ROOT,
                env=environment,
            )
            values = dict(
                line.split("=", 1)
                for line in output.read_text().splitlines()
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
                    "GITHUB_REF": "refs/heads/epic/8056-modules-wiring",
                },
                {"mode": "reviewed-epic", "tag": "v0.4.2", "version": "0.4.2", "reviewed_pr": "16"},
            ),
        )
        for inputs, expected in valid:
            with self.subTest(inputs=inputs):
                result, outputs = self.resolve_release(**inputs)
                self.assertEqual(result.returncode, 0, bash_diagnostic(result))
                for key, value in expected.items():
                    self.assertEqual(outputs.get(key), value)

        epic_ref = "refs/heads/epic/8056-modules-wiring"
        invalid = (
            ({"BUMP": ""}, "main release requires bump"),
            ({"BUMP": "wat"}, "main release requires bump"),
            ({"BUMP": "patch", "TAG": "v0.4.2"}, "only valid in reviewed-epic mode"),
            (
                {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.2", "VERSION": "0.4.2", "GITHUB_REF": epic_ref},
                "reviewed-pr must be a positive canonical PR number",
            ),
            (
                {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.2", "VERSION": "0.4.2", "REVIEWED_PR": "0", "GITHUB_REF": epic_ref},
                "reviewed-pr must be a positive canonical PR number",
            ),
            (
                {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.2", "VERSION": "0.4.2", "REVIEWED_PR": "016", "GITHUB_REF": epic_ref},
                "reviewed-pr must be a positive canonical PR number",
            ),
            (
                {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.3", "VERSION": "0.4.2", "REVIEWED_PR": "16", "GITHUB_REF": epic_ref},
                "reviewed-epic tag and version must match",
            ),
            (
                {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.2", "VERSION": "0.4.2", "REVIEWED_PR": "16", "GITHUB_EVENT_NAME": "push", "GITHUB_REF": epic_ref},
                "releases must be manually dispatched",
            ),
            (
                {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.2", "VERSION": "0.4.2", "REVIEWED_PR": "16", "GITHUB_REF": "refs/heads/main"},
                "reviewed-epic releases must be dispatched from epic/8056-modules-wiring",
            ),
            (
                {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.2", "VERSION": "0.4.2", "REVIEWED_PR": "16", "GITHUB_REF": "refs/heads/feature"},
                "reviewed-epic releases must be dispatched from epic/8056-modules-wiring",
            ),
            (
                {"RELEASE_MODE": "reviewed-epic", "TAG": "v0.4.2", "VERSION": "0.4.2", "REVIEWED_PR": "16", "GITHUB_REF": "refs/tags/v0.4.2"},
                "reviewed-epic releases must be dispatched from epic/8056-modules-wiring",
            ),
            (
                {"RELEASE_MODE": "main", "BUMP": "patch", "GITHUB_REF": epic_ref},
                "main releases must be dispatched from main",
            ),
            ({"RELEASE_MODE": "unknown", "BUMP": "patch"}, "release_mode must be main or reviewed-epic"),
        )
        for inputs, expected_error in invalid:
            with self.subTest(invalid=inputs):
                result, _ = self.resolve_release(**inputs)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(expected_error, result.stderr, bash_diagnostic(result))

        release = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("reviewed-module-release:", release)
        self.assertIn(
            "module-reviewed-epic-release.yml@" + RELEASE_PIN,
            release,
        )
        self.assertIn("module-release:", release)
        self.assertIn("needs.resolve.outputs.mode == 'main'", release)
        self.assertIn("refs/heads/epic/8056-modules-wiring", release)
        self.assertIn("ref: ${{ github.sha }}", release)
        self.assertIn("github.event_name == 'workflow_dispatch'", release)
        self.assertIn("github.ref == 'refs/heads/main' && needs.resolve.outputs.mode == 'main'", release)
        self.assertIn("github.ref == 'refs/heads/epic/8056-modules-wiring' && needs.resolve.outputs.mode == 'reviewed-epic'", release)

    def test_git_bash_selection_ignores_poisoned_bare_bash_path(self):
        with tempfile.TemporaryDirectory(prefix="bash-selection-") as scratch:
            scratch = Path(scratch)
            poison = scratch / "poison"
            git_root = scratch / "Git"
            git_bin = git_root / "bin"
            git_cmd = git_root / "cmd"
            poison.mkdir()
            git_bin.mkdir(parents=True)
            git_cmd.mkdir()

            executable_suffix = ".exe" if os.name == "nt" else ""
            poisoned_bash = poison / f"bash{executable_suffix}"
            poisoned_bash.write_bytes(b"poisoned bare Bash selector")
            poisoned_bash.chmod(0o755)
            git_executable = git_cmd / f"git{executable_suffix}"
            git_executable.write_bytes(b"Git executable selector fixture")
            git_executable.chmod(0o755)
            selected_bash = git_bin / "bash.exe"
            selected_bash.write_bytes(b"selected Git Bash fixture")
            path = os.pathsep.join((str(poison), str(git_cmd)))

            bare_bash = shutil.which("bash", path=path)
            self.assertIsNotNone(bare_bash)
            # Windows PATHEXT lookup may return .EXE while the fixture is .exe.
            self.assertTrue(Path(bare_bash).samefile(poisoned_bash))
            self.assertFalse(Path(bare_bash).samefile(selected_bash))
            selected = git_bash_executable(path=path, windows=True)
            self.assertTrue(Path(selected).samefile(selected_bash))
            self.assertFalse(Path(selected).samefile(poisoned_bash))
            command = bash_command("scripts/resolve-release-mode.sh", path=path, windows=True)
            self.assertTrue(Path(command[0]).samefile(selected_bash))
            self.assertEqual(command[1], "scripts/resolve-release-mode.sh")

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
                            self.assertEqual(dependency["version"], "=0.3.11")

    def test_nested_gitlink_matches_the_published_sdk_revision(self):
        rev = subprocess.run(
            ["git", "-C", str(ROOT / "vendor/tinybus"), "rev-parse", "HEAD"],
            text=True,
            capture_output=True,
            check=True,
        ).stdout.strip()
        self.assertEqual(rev, SDK_REV)

    def test_cross_workspace_local_dependencies_keep_exact_versions(self):
        client_bus = manifest("crates/tinymemes-client/Cargo.toml")["dependencies"][
            "tinymemes-bus"
        ]
        runner = manifest("proof-workspaces/remix-markers/runner/Cargo.toml")[
            "dependencies"
        ]
        self.assertEqual(client_bus["version"], "0.4.2")
        self.assertEqual(runner["tinymemes-bus"]["version"], "0.4.2")
        self.assertEqual(runner["tinymemes-client"]["version"], "0.4.2")

    def test_private_marker_runner_declares_repository_license(self):
        runner = manifest("proof-workspaces/remix-markers/runner/Cargo.toml")
        self.assertFalse(runner["package"]["publish"])
        self.assertEqual(runner["package"]["license"], "GPL-3.0-only")

    def test_sdk_patch_source_is_explicitly_allowlisted(self):
        policy = tomllib.loads((ROOT / "deny.toml").read_text())
        self.assertEqual(policy["sources"]["unknown-git"], "deny")
        self.assertIn(
            "https://github.com/senamakel/tinybus",
            policy["sources"]["allow-git"],
        )

    def test_standalone_tsd_graph_uses_the_same_tinybus_gitlink(self):
        self.assertIn("vendor/tinybus", manifest("Cargo.toml")["workspace"]["exclude"])
        for relative_root, lock_path in (
            ("Cargo.toml", "Cargo.lock"),
            (
                "proof-workspaces/remix-markers/Cargo.toml",
                "proof-workspaces/remix-markers/Cargo.lock",
            ),
        ):
            with self.subTest(root=relative_root):
                patch = manifest(relative_root)["patch"][
                    "https://github.com/tinyhumansai/tinybus"
                ]
                self.assertEqual(
                    patch,
                    {
                        "tinybus": {
                            "version": "=0.3.11",
                            "git": "https://github.com/senamakel/tinybus",
                            "rev": SDK_REV,
                        },
                        "tinybus-module": {
                            "version": "=0.3.11",
                            "git": "https://github.com/senamakel/tinybus",
                            "rev": SDK_REV,
                        },
                    },
                )
                lock = tomllib.loads((ROOT / lock_path).read_text())
                packages = [
                    package
                    for package in lock["package"]
                    if package["name"] in {"tinybus", "tinybus-module"}
                ]
                self.assertEqual(len(packages), 2)
                self.assertEqual({package["version"] for package in packages}, {"0.3.11"})
                self.assertEqual(
                    {package["source"] for package in packages},
                    {
                        f"git+https://github.com/senamakel/tinybus?rev={SDK_REV}#{SDK_REV}"
                    },
                )

    def test_storage_client_keeps_an_explicit_compatible_sdk_edge(self):
        dependency = manifest("crates/tinymemes-module/Cargo.toml")["dependencies"][
            "tinystoragedrivers-bus"
        ]
        self.assertEqual(dependency["version"], "1.9.0")
        self.assertEqual(dependency["rev"], TSD_REV)
        self.assertNotIn("tag", dependency)
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
                            self.assertEqual(dependency.get("version"), "1.9.0")
                            self.assertEqual(dependency.get("rev"), TSD_REV)
                            self.assertNotIn("tag", dependency)

    def test_contract_storage_schema_stays_opt_in_and_ports_only(self):
        cargo = manifest("crates/tinymemes-bus/Cargo.toml")
        self.assertEqual(cargo["features"]["default"], [])
        self.assertIn("storage", cargo["features"])
        dependency = cargo["dependencies"]["tinystoragedrivers-core"]
        self.assertEqual(dependency["version"], "1.9.0")
        self.assertEqual(dependency["rev"], TSD_REV)
        self.assertNotIn("tag", dependency)
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
        self.assertIn('bash scripts/update-release-versions.sh "$PWD" "$NEXT_VERSION"', release)
        release_commit_inputs = release.split("          git add ", 1)[1].split(
            "\n          git commit", 1
        )[0]
        for relative_path in (
            "Cargo.toml",
            "Cargo.lock",
            "crates/tinymemes-module/Cargo.toml",
            "crates/tinymemes-bus/Cargo.toml",
            "crates/tinymemes-client/Cargo.toml",
            "proof-workspaces/remix-markers/runner/Cargo.toml",
            "proof-workspaces/remix-markers/Cargo.lock",
            "proof-workspaces/bus-contract/Cargo.lock",
        ):
            with self.subTest(staged=relative_path):
                self.assertIn(relative_path, release_commit_inputs)

    def test_release_version_bump_updates_only_package_and_local_path_versions(self):
        copied_manifests = {
            "Cargo.toml": {"package": True, "dependencies": ["tinymemes-bus"]},
            "crates/tinymemes-module/Cargo.toml": {"package": True},
            "crates/tinymemes-bus/Cargo.toml": {"package": True},
            "crates/tinymemes-client/Cargo.toml": {
                "package": True,
                "dependencies": ["tinymemes-bus"],
            },
            "proof-workspaces/remix-markers/runner/Cargo.toml": {
                "package": False,
                "dependencies": ["tinymemes-bus", "tinymemes-client"],
            },
        }
        versions = ("0.4.3", "0.5.0", "1.0.0")
        (ROOT / "target").mkdir(exist_ok=True)
        for next_version in versions:
            with self.subTest(next_version=next_version):
                with tempfile.TemporaryDirectory(
                    prefix="release-version-bump-", dir=ROOT / "target"
                ) as scratch:
                    scratch_root = Path(scratch)
                    before = {}
                    for relative_path in copied_manifests:
                        destination = scratch_root / relative_path
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copy2(ROOT / relative_path, destination)
                        before[relative_path] = tomllib.loads(destination.read_text())

                    result = run_bash(
                        "scripts/update-release-versions.sh",
                        scratch_root.relative_to(ROOT).as_posix(),
                        next_version,
                        cwd=ROOT,
                    )
                    self.assertEqual(result.returncode, 0, bash_diagnostic(result))

                    for relative_path, expected_updates in copied_manifests.items():
                        after = tomllib.loads((scratch_root / relative_path).read_text())
                        expected = before[relative_path]
                        if expected_updates["package"]:
                            expected["package"]["version"] = next_version
                        for dependency in expected_updates.get("dependencies", []):
                            expected["dependencies"][dependency]["version"] = next_version
                        self.assertEqual(after, expected, relative_path)

    def test_reviewed_release_caller_matches_pinned_reusable_input_types(self):
        actionlint = shutil.which("actionlint")
        if actionlint is None:
            self.skipTest("actionlint is required to validate reusable workflow input types")

        release = (ROOT / ".github/workflows/release.yml").read_text()
        release_pin = f"module-reviewed-epic-release.yml@{RELEASE_PIN}"
        remote_uses = f"tinyhumansai/.github/.github/workflows/{release_pin}"
        local_uses = "./.github/workflows/reusable.yml"
        self.assertIn(remote_uses, release)

        # actionlint can check typed inputs only when the reusable workflow is local.
        # This is the reviewed-pr interface declared by the pinned shared workflow.
        reusable = """name: Reviewed module epic release
on:
  workflow_call:
    inputs:
      tag:
        type: string
        required: true
      version:
        type: string
        required: true
      reviewed-pr:
        type: number
        required: true
jobs:
  contract:
    runs-on: ubuntu-latest
    steps:
      - run: echo \"${{ inputs.reviewed-pr }}\"
"""
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="release-input-type-", dir=ROOT / "target") as scratch:
            project = Path(scratch)
            workflows = project / ".github/workflows"
            workflows.mkdir(parents=True)
            (workflows / "release.yml").write_text(release.replace(remote_uses, local_uses))
            (workflows / "reusable.yml").write_text(reusable)
            subprocess.run(["git", "init", "-q"], cwd=project, check=True)
            result = subprocess.run(
                [actionlint, ".github/workflows/release.yml"],
                cwd=project,
                text=True,
                capture_output=True,
                check=False,
            )

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

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
