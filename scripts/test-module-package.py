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


ROOT = Path(__file__).resolve().parent.parent
VERSION = "0.4.2"
HOST = "ubuntu-24.04-x86_64"


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
        cargo.write_text("#!/bin/sh\nexec python3 \"$SPY_SCRIPT\" \"$@\"\n")
        cargo.chmod(0o755)
        python = spy_bin / "python3"
        python.write_text("#!/bin/sh\nexec \"$SPY_PYTHON\" \"$@\"\n")
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
            verifier = ROOT / "scripts/verify-module.sh"
            accepted = subprocess.run(
                ["bash", str(verifier), "--archive", str(archive)],
                cwd=ROOT,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(accepted.returncode, 0, accepted.stderr)
            calls = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            e2e_calls = [
                call
                for call in calls
                if call["args"][:1] == ["test"]
                or call["args"][:1] == ["run"]
            ]
            self.assertTrue(e2e_calls, f"no Cargo native checks were requested: {calls}")
            self.assertTrue(all(call["sha256"] == digest for call in e2e_calls), calls)
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
            rejected = subprocess.run(
                ["bash", str(verifier), "--archive", str(archive)],
                cwd=ROOT,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertNotEqual(rejected.returncode, 0)
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
            verifier = ROOT / "scripts/verify-module.sh"
            accepted = subprocess.run(
                ["bash", str(verifier), "--archive", str(archive)],
                cwd=ROOT,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(accepted.returncode, 0, accepted.stderr)

            files["modules.toml"] = b'"libtinymemes_module.so" = "' + b"0" * 64 + b'"\n'
            with tarfile.open(archive, "w:gz") as package:
                for name, body in files.items():
                    path = scratch / name
                    path.write_bytes(body)
                    package.add(path, arcname=name)
            rejected = subprocess.run(
                ["bash", str(verifier), "--archive", str(archive)],
                cwd=ROOT,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertNotEqual(rejected.returncode, 0)
            self.assertIn("does not pin", rejected.stderr)

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
            self.assertEqual(verified.returncode, 0, verified.stderr)
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

    def test_compiled_loader_windows_fixture_uses_private_inherited_acl(self):
        source = (ROOT / "crates/tinymemes-module/tests/compiled_remix.rs").read_text()
        helper = source.split("fn secure_windows_fixture_directory", 1)[1].split("\n    }", 1)[0]
        self.assertIn("#[cfg(windows)]", source.split("fn secure_windows_fixture_directory", 1)[0][-40:])
        self.assertIn('"/inheritance:r", "/grant:r"', helper)
        self.assertIn('format!("*{}:(OI)(CI)F"', helper)
        self.assertIn("secure_windows_fixture_directory(&path)", source)


if __name__ == "__main__":
    unittest.main()
