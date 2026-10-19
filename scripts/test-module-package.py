"""Test archive checks and verifier-to-loader wiring with a Cargo spy only."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from unittest.mock import patch
sys.dont_write_bytecode = True
from test_support import bash_diagnostic, run_bash


ROOT = Path(__file__).resolve().parent.parent
VERSION = "0.4.2"
HOST = "ubuntu-24.04-x86_64"


def write_shell_shim(path, contents):
    path.write_text(contents, newline="\n")


class PackageVerification(unittest.TestCase):
    def install_cargo_spy(self, scratch, environment):
        spy_bin = scratch / "bin"
        spy_bin.mkdir()
        spy_script = scratch / "cargo-spy.py"
        spy_script.write_text(
            "import hashlib, json, os, pathlib, sys\n"
            "args = sys.argv[1:]\n"
            "module = os.environ.get('TINYMEMES_MODULE')\n"
            "digest = hashlib.sha256(pathlib.Path(module).read_bytes()).hexdigest() if module else None\n"
            "record = {'args': args, 'module': module, 'sha256': digest}\n"
            "with open(os.environ['SPY_LOG'], 'a') as log: log.write(json.dumps(record) + '\\n')\n"
            "if os.environ.get('SPY_FAIL') in args: sys.exit(42)\n"
            "if args and args[0] == 'metadata': print(json.dumps({'target_directory': os.environ['SPY_TARGET']}))\n"
        )
        cargo = spy_bin / "cargo"
        write_shell_shim(cargo, "#!/bin/sh\nexec python3 \"$SPY_SCRIPT\" \"$@\"\n")
        cargo.chmod(0o755)
        python = spy_bin / "python3"
        write_shell_shim(python, "#!/bin/sh\nexec \"$SPY_PYTHON\" \"$@\"\n")
        python.chmod(0o755)
        log = scratch / "cargo-calls.jsonl"
        environment.update(
            SPY_LOG=str(log),
            SPY_PYTHON=sys.executable,
            SPY_SCRIPT=str(spy_script),
            SPY_TARGET=str(scratch / "target"),
            PATH=f"{spy_bin}{os.pathsep}{environment['PATH']}",
        )
        return log

    def test_generated_executable_shims_keep_lf_under_windows_text_translation(self):
        def windows_write_text(path, data, encoding=None, errors=None, newline=None):
            effective_newline = "\r\n" if newline is None else newline
            with path.open("w", encoding=encoding, errors=errors, newline=effective_newline) as stream:
                return stream.write(data)

        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="module-shim-newlines-", dir=ROOT / "target") as temp:
            scratch = Path(temp)
            environment = dict(os.environ)
            with patch.object(Path, "write_text", windows_write_text):
                self.install_cargo_spy(scratch, environment)

            for shim_name in ("cargo", "python3"):
                with self.subTest(shim=shim_name):
                    contents = (scratch / "bin" / shim_name).read_bytes()
                    self.assertTrue(contents.startswith(b"#!/bin/sh\n"), contents)
                    self.assertNotIn(b"\r", contents)

    def run_unix_verifier(self, archive, environment):
        archive_arg = Path(os.path.relpath(archive, ROOT)).as_posix()
        return run_bash(
            "scripts/verify-module.sh",
            "--archive",
            archive_arg,
            cwd=ROOT,
            env=environment,
        )

    @unittest.skipUnless(os.name == "posix", "Unix archive verifier runs on Unix CI")
    def test_verifier_passes_extracted_library_to_e2e_and_propagates_failure(self):
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="module-loader-spy-", dir=ROOT / "target") as temp:
            scratch = Path(temp)
            library = b"fixture native module bytes"
            digest = hashlib.sha256(library).hexdigest()
            archive = scratch / f"tinymemes-module-{VERSION}-{HOST}.tar.gz"
            with tarfile.open(archive, "w:gz") as package:
                for name, body in {
                    "libtinymemes_module.so": library,
                    "LICENSE": b"license fixture",
                    "README.md": b"readme fixture",
                    "modules.toml": f'"libtinymemes_module.so" = "{digest}"\n'.encode(),
                }.items():
                    path = scratch / name
                    path.write_bytes(body)
                    package.add(path, arcname=name)

            environment = dict(os.environ)
            environment.update(
                MODULE_ASSET="tinymemes-module",
                MODULE_VERSION=VERSION,
                MODULE_HOST=HOST,
            )
            log = self.install_cargo_spy(scratch, environment)
            accepted = self.run_unix_verifier(archive, environment)
            self.assertEqual(accepted.returncode, 0, bash_diagnostic(accepted))
            calls = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            e2e_calls = [
                call
                for call in calls
                if call["args"][:1] == ["test"]
                or call["args"][:1] == ["run"]
            ]
            self.assertTrue(e2e_calls, f"no Cargo native checks were requested: {calls}")
            self.assertTrue(all(call["sha256"] == digest for call in e2e_calls), calls)
            staged_paths = {Path(call["module"]) for call in e2e_calls}
            self.assertEqual(len(staged_paths), 1, calls)
            staged_path = next(iter(staged_paths))
            self.assertTrue(staged_path.parent.name.startswith("module-e2e-private-"), calls)
            self.assertFalse(any(call["args"][:1] == ["build"] for call in calls), calls)
            self.assertTrue(
                any("compiled_remix" in call["args"] for call in calls),
                calls,
            )
            self.assertTrue(
                any("compiled_state" in call["args"] for call in calls),
                calls,
            )
            self.assertTrue(
                any("tinymemes-remix-marker-proof" in call["args"] for call in calls),
                calls,
            )

            log.write_text("")
            environment["SPY_FAIL"] = "compiled_remix"
            rejected = self.run_unix_verifier(archive, environment)
            self.assertNotEqual(rejected.returncode, 0, bash_diagnostic(rejected))
            failed_calls = [json.loads(line) for line in log.read_text().splitlines()]
            self.assertTrue(any("compiled_remix" in call["args"] for call in failed_calls))

    @unittest.skipUnless(os.name == "posix", "Unix archive verifier runs on Unix CI")
    def test_unix_verifier_checks_digest_and_propagates_native_spy_results(self):
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="module-package-", dir=ROOT / "target") as temp:
            scratch = Path(temp)
            library = b"fixture native module bytes"
            digest = hashlib.sha256(library).hexdigest()
            files = {
                "libtinymemes_module.so": library,
                "LICENSE": b"license fixture",
                "README.md": b"readme fixture",
                "modules.toml": f'"libtinymemes_module.so" = "{digest}"\n'.encode(),
            }
            archive = scratch / f"tinymemes-module-{VERSION}-{HOST}.tar.gz"
            with tarfile.open(archive, "w:gz") as package:
                for name, body in files.items():
                    path = scratch / name
                    path.write_bytes(body)
                    package.add(path, arcname=name)

            environment = dict(os.environ)
            environment.update(
                MODULE_ASSET="tinymemes-module",
                MODULE_VERSION=VERSION,
                MODULE_HOST=HOST,
            )
            self.install_cargo_spy(scratch, environment)
            accepted = self.run_unix_verifier(archive, environment)
            self.assertEqual(accepted.returncode, 0, bash_diagnostic(accepted))

            files["modules.toml"] = b'"libtinymemes_module.so" = "' + b"0" * 64 + b'"\n'
            with tarfile.open(archive, "w:gz") as package:
                for name, body in files.items():
                    path = scratch / name
                    path.write_bytes(body)
                    package.add(path, arcname=name)
            rejected = self.run_unix_verifier(archive, environment)
            self.assertNotEqual(rejected.returncode, 0, bash_diagnostic(rejected))
            self.assertIn("does not pin", rejected.stderr, bash_diagnostic(rejected))

    @unittest.skipUnless(shutil.which("pwsh"), "PowerShell is only installed on Windows CI")
    def test_windows_verifier_checks_digest_and_native_spy_results(self):
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="module-package-", dir=ROOT / "target") as temp:
            scratch = Path(temp)
            library = b"fixture Windows native module bytes"
            digest = hashlib.sha256(library).hexdigest()
            archive = scratch / f"tinymemes-module-{VERSION}-windows-2025-x86_64.zip"
            with zipfile.ZipFile(archive, "w") as package:
                package.writestr("tinymemes_module.dll", library)
                package.writestr("LICENSE", b"license fixture")
                package.writestr("README.md", b"readme fixture")
                package.writestr("modules.toml", f'"tinymemes_module.dll" = "{digest}"\n')

            environment = dict(os.environ)
            environment.update(
                MODULE_ASSET="tinymemes-module",
                MODULE_VERSION=VERSION,
                MODULE_HOST="windows-2025-x86_64",
            )
            log = self.install_cargo_spy(scratch, environment)
            selected_bash = None
            selected_bash_version = None
            if os.name == "posix":
                source_bash = shutil.which("bash")
                self.assertIsNotNone(source_bash)
                bash_dir = scratch / "bash-selection"
                bash_dir.mkdir()
                selected_bash = bash_dir / "bash"
                shutil.copyfile(source_bash, selected_bash)
                selected_bash.chmod(0o755)
                environment["PATH"] = f"{bash_dir}{os.pathsep}{environment['PATH']}"

                matches = subprocess.run(
                    [
                        "pwsh",
                        "-NoLogo",
                        "-NoProfile",
                        "-NonInteractive",
                        "-Command",
                        "$commands = @(Get-Command bash -CommandType Application); $commands | ForEach-Object Source",
                    ],
                    env=environment,
                    text=True,
                    capture_output=True,
                    check=False,
                )
                self.assertEqual(
                    matches.returncode,
                    0,
                    f"Bash Application lookup failed\nstdout: {matches.stdout!r}\nstderr: {matches.stderr!r}",
                )
                match_paths = matches.stdout.splitlines()
                self.assertGreaterEqual(len(match_paths), 2, matches.stdout)
                self.assertTrue(Path(match_paths[0]).samefile(selected_bash), matches.stdout)
                version = subprocess.run(
                    [str(selected_bash), "--version"],
                    text=True,
                    capture_output=True,
                    check=False,
                )
                self.assertEqual(version.returncode, 0, version.stderr)
                selected_bash_version = version.stdout.splitlines()[0]

            verified = subprocess.run(
                [
                    "pwsh",
                    "-NoLogo",
                    "-NoProfile",
                    "-File",
                    str(ROOT / "scripts/verify-module.ps1"),
                    "-Archive",
                    str(archive),
                ],
                cwd=ROOT,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(
                verified.returncode,
                0,
                f"PowerShell verifier failed\nstdout: {verified.stdout!r}\nstderr: {verified.stderr!r}",
            )
            if selected_bash is not None:
                self.assertIn(
                    f"Using Bash executable: {selected_bash} ({selected_bash_version})",
                    verified.stdout,
                )
            calls = [json.loads(line) for line in log.read_text().splitlines()]
            loader_calls = [
                call for call in calls
                if call["args"][:1] in (["test"], ["run"])
            ]
            self.assertTrue(loader_calls, f"no native checks were requested: {calls}")
            self.assertTrue(all(call["sha256"] == digest for call in loader_calls), calls)
            self.assertFalse(any(call["args"][:1] == ["build"] for call in calls), calls)
            self.assertTrue(any("compiled_remix" in call["args"] for call in calls), calls)
            self.assertTrue(any("compiled_state" in call["args"] for call in calls), calls)
            self.assertTrue(any("tinymemes-remix-marker-proof" in call["args"] for call in calls), calls)

    def test_windows_acl_setup_is_gated_to_windows_hosts(self):
        verifier = (ROOT / "scripts/verify-module.ps1").read_text()
        self.assertIn("if ($IsWindows)", verifier)
        gate = verifier.index("if ($IsWindows)")
        identity = verifier.index("WindowsIdentity]::GetCurrent()")
        acl = verifier.index("Set-Acl", identity)
        self.assertLess(gate, identity)
        self.assertLess(identity, acl)

    def test_native_loader_fixtures_use_private_windows_directory_acl(self):
        support = (ROOT / "crates/tinymemes-module/tests/support/native_artifact.rs").read_text()
        helper = support.split("pub fn secure_fixture_directory", 1)[1].split("\n}", 1)[0]
        self.assertIn('"/inheritance:r", "/grant:r"', helper)
        self.assertIn('format!("*{}:(OI)(CI)F"', helper)

        compiled = (ROOT / "crates/tinymemes-module/tests/compiled_remix.rs").read_text()
        self.assertIn("super::native_artifact::secure_fixture_directory(&path)", compiled)

        markers = (ROOT / "crates/tinymemes-module/tests/remix_markers.rs").read_text()
        fixture_setup = markers.split("impl Fixture {", 1)[1].split("    fn proxy_to", 1)[0]
        secure = fixture_setup.index("native_artifact::secure_fixture_directory(directory.path())")
        copy = fixture_setup.index("std::fs::copy(&artifact, &staged)")
        self.assertLess(secure, copy)

    @unittest.skipUnless(shutil.which("pwsh"), "PowerShell is only installed on Windows CI")
    def test_git_bash_resolution_handles_cmd_and_mingw_bin_git_layouts(self):
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="git-bash-layouts-", dir=ROOT / "target") as temp:
            scratch = Path(temp)
            git_root = scratch / "Git"
            bash = git_root / "bin" / "bash.exe"
            cmd_git = git_root / "cmd" / "git.exe"
            mingw_git = git_root / "mingw64" / "bin" / "git.exe"
            for path in (bash, cmd_git, mingw_git):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.touch()

            resolver = ROOT / "scripts/resolve-git-bash.ps1"
            probe = scratch / "probe-git-bash.ps1"
            probe.write_text(
                "$ErrorActionPreference = 'Stop'\n"
                f". '{resolver.as_posix()}'\n"
                f"Resolve-GitBashExecutable -GitExecutable '{cmd_git.as_posix()}'\n"
                f"Resolve-GitBashExecutable -GitExecutable '{mingw_git.as_posix()}'\n",
                newline="\n",
            )
            result = subprocess.run(
                ["pwsh", "-NoLogo", "-NoProfile", "-NonInteractive", "-File", str(probe)],
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(
                result.returncode,
                0,
                f"Git Bash path resolution failed\nstdout: {result.stdout!r}\nstderr: {result.stderr!r}",
            )
            resolved = result.stdout.splitlines()
            self.assertEqual(len(resolved), 2, result.stdout)
            self.assertTrue(all(Path(candidate).samefile(bash) for candidate in resolved), resolved)

    def test_module_e2e_stages_one_hashed_artifact_in_a_private_windows_directory(self):
        source = (ROOT / "scripts/test-module-e2e.sh").read_text()
        self.assertIn('module-e2e-private-XXXXXX', source)
        self.assertIn("MSYS2_ARG_CONV_EXCL='*' icacls", source)
        self.assertIn('source_hash="$(file_hash "$source_path")"', source)
        self.assertIn('staged_hash="$(file_hash "$staged_module")"', source)
        self.assertIn('[[ "$source_hash" == "$staged_hash" ]]', source)
        self.assertIn('staged_env_path="$(cygpath -w "$staged_module")"', source)
        self.assertIn('TINYMEMES_MODULE="$staged_env_path" cargo test', source)
        self.assertIn('TINYMEMES_MODULE="$staged_env_path" cargo run', source)
        self.assertIn('scripts/test-remix-markers.sh', source)
        self.assertNotIn('chmod 700 "$root/target/debug', source)

        verifier = (ROOT / "scripts/verify-module.ps1").read_text()
        self.assertIn("Resolve-GitBashExecutable -GitExecutable $git.Source", verifier)
        self.assertIn("resolve-git-bash.ps1", verifier)
        self.assertIn("Get-Command bash -CommandType Application", verifier)
        self.assertIn("& $bash 'scripts/test-module-e2e.sh'", verifier)

    @unittest.skipUnless(os.name == "posix", "Native-command boundary spies use POSIX executable shims")
    def test_windows_icacls_receives_native_args_with_command_scoped_exclusion(self):
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="module-acl-boundary-", dir=ROOT / "target") as temp:
            scratch = Path(temp)
            spy_bin = scratch / "bin"
            spy_bin.mkdir()
            source = scratch / "tinymemes_module.dll"
            source.write_bytes(b"Windows ACL boundary fixture")
            calls = scratch / "native-calls.jsonl"

            uname = spy_bin / "uname"
            write_shell_shim(uname, "#!/bin/sh\necho MINGW_NT-10.0-1\n")
            uname.chmod(0o755)
            cygpath = spy_bin / "cygpath"
            write_shell_shim(cygpath, "#!/bin/sh\nshift\nprintf '%s\\n' \"$1\"\n")
            cygpath.chmod(0o755)
            powershell = spy_bin / "powershell.exe"
            write_shell_shim(powershell, "#!/bin/sh\necho S-1-5-21-12345\n")
            powershell.chmod(0o755)

            native_spy = scratch / "native-spy.py"
            native_spy.write_text(
                "import json, os, pathlib, sys\n"
                "record = {'command': pathlib.Path(sys.argv[0]).name, 'args': sys.argv[1:], "
                "'MSYS2_ARG_CONV_EXCL': os.environ.get('MSYS2_ARG_CONV_EXCL'), "
                "'TINYMEMES_MODULE': os.environ.get('TINYMEMES_MODULE')}\n"
                "with open(os.environ['NATIVE_CALLS'], 'a') as calls: "
                "calls.write(json.dumps(record) + '\\n')\n"
            )
            for name in ("icacls", "cargo"):
                shim = spy_bin / name
                write_shell_shim(shim, f"#!{sys.executable}\nexec(open({str(native_spy)!r}).read())\n")
                shim.chmod(0o755)

            environment = dict(os.environ)
            environment.update(
                TINYMEMES_MODULE=str(source),
                NATIVE_CALLS=str(calls),
                MSYS2_ARG_CONV_EXCL="caller-sentinel",
                PATH=f"{spy_bin}{os.pathsep}{environment['PATH']}",
            )
            result = run_bash(
                "scripts/test-module-e2e.sh",
                cwd=ROOT,
                env=environment,
            )
            self.assertEqual(result.returncode, 0, bash_diagnostic(result))
            records = [json.loads(line) for line in calls.read_text().splitlines()]
            acl_calls = [record for record in records if record["command"] == "icacls"]
            cargo_calls = [record for record in records if record["command"] == "cargo"]
            self.assertEqual(len(acl_calls), 1, records)
            acl = acl_calls[0]
            self.assertRegex(acl["args"][0], r"module-e2e-private-[^/\\]+$")
            self.assertEqual(
                acl["args"][1:],
                ["/inheritance:r", "/grant:r", "*S-1-5-21-12345:(OI)(CI)F"],
                records,
            )
            self.assertEqual(acl["MSYS2_ARG_CONV_EXCL"], "*", records)
            self.assertEqual(len(cargo_calls), 3, records)
            self.assertTrue(
                all(record["MSYS2_ARG_CONV_EXCL"] == "caller-sentinel" for record in cargo_calls),
                records,
            )
            self.assertEqual(
                {record["TINYMEMES_MODULE"] for record in cargo_calls},
                {str(Path(acl["args"][0]) / "tinymemes_module.dll")},
                records,
            )

    @unittest.skipUnless(os.name == "posix", "The corrupting cp fixture uses a POSIX script shim")
    def test_module_e2e_rejects_staged_hash_mismatch_before_native_consumers(self):
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="module-stage-mismatch-", dir=ROOT / "target") as temp:
            scratch = Path(temp)
            source = scratch / "libtinymemes_module.so"
            source.write_bytes(b"source module bytes")

            spy_bin = scratch / "bin"
            spy_bin.mkdir()
            real_cp = shutil.which("cp")
            self.assertIsNotNone(real_cp)
            corrupting_cp = spy_bin / "cp"
            write_shell_shim(
                corrupting_cp,
                f"#!{sys.executable}\n"
                "import os, pathlib, subprocess, sys\n"
                "subprocess.run([os.environ['REAL_CP'], *sys.argv[1:]], check=True)\n"
                "with pathlib.Path(sys.argv[-1]).open('ab') as staged: staged.write(b'corruption')\n"
            )
            corrupting_cp.chmod(0o755)
            cargo_log = scratch / "cargo-was-called"
            cargo = spy_bin / "cargo"
            write_shell_shim(cargo, "#!/bin/sh\necho called >> \"$CARGO_SPY_LOG\"\n")
            cargo.chmod(0o755)

            environment = dict(os.environ)
            environment.update(
                TINYMEMES_MODULE=str(source),
                REAL_CP=real_cp,
                CARGO_SPY_LOG=str(cargo_log),
                PATH=f"{spy_bin}{os.pathsep}{environment['PATH']}",
            )
            result = run_bash(
                "scripts/test-module-e2e.sh",
                cwd=ROOT,
                env=environment,
            )
            self.assertNotEqual(result.returncode, 0, bash_diagnostic(result))
            self.assertIn("staged module artifact hash differs", result.stderr, bash_diagnostic(result))
            self.assertFalse(cargo_log.exists(), bash_diagnostic(result))


if __name__ == "__main__":
    unittest.main()
