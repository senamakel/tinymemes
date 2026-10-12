"""Exercise the shell runner without Cargo, including empty forwarded arguments."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


class RunnerArguments(unittest.TestCase):
    def run_runner(self, extra):
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="runner-arguments-", dir=ROOT / "target") as scratch:
            scratch = Path(scratch)
            (scratch / "cargo").write_text(
                '#!/bin/sh\n'
                'printf "CALL\\n" >> "$MOCK_LOG"\n'
                'for arg do printf "ARG:%s\\n" "$arg" >> "$MOCK_LOG"; done\n'
                'printf "MODULE:%s\\n" "$TINYMEMES_MODULE" >> "$MOCK_LOG"\n'
                'printf \'{"target_directory":"unused"}\\n\'\n'
            )
            (scratch / "python3").write_text(
                '#!/bin/sh\ncat >/dev/null\nprintf "%s\\n" "$MOCK_TARGET"\n'
            )
            (scratch / "uname").write_text('#!/bin/sh\nprintf "Linux\\n"\n')
            for command in ("cargo", "python3", "uname"):
                (scratch / command).chmod(0o755)
            log = scratch / "calls.log"
            environment = dict(os.environ)
            environment.update(
                PATH=f"{scratch}:/usr/local/bin:/usr/bin:/bin",
                MOCK_LOG=str(log),
                MOCK_TARGET=str(scratch / "build"),
            )
            shell = os.environ.get("REMIX_MARKER_TEST_SHELL", "bash")
            runner = os.environ.get(
                "REMIX_MARKER_TEST_RUNNER",
                str(ROOT / "scripts/test-remix-markers.sh"),
            )
            result = subprocess.run(
                [shell, runner, *extra],
                env=environment, text=True, capture_output=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            calls = [block.splitlines() for block in log.read_text().split("CALL\n")[1:]]
            self.assertEqual(len(calls), 3)
            expected = [
                ["build", "--locked", "-p", "tinymemes-module"],
                ["metadata", "--locked", "--no-deps", "--format-version", "1"],
                ["test", "--locked", "--manifest-path",
                 "proof-workspaces/remix-markers/Cargo.toml",
                 "-p", "tinymemes-remix-marker-proof"],
            ]
            for call, command in zip(calls, expected):
                self.assertEqual(
                    [line[4:] for line in call if line.startswith("ARG:")],
                    [*extra, *command],
                )
            self.assertEqual(
                calls[2][-1], f"MODULE:{scratch}/build/debug/libtinymemes_module.so"
            )

    def test_no_optional_arguments_reaches_every_cargo_command(self):
        self.run_runner([])

    def test_optional_arguments_preserve_spaces_and_order_for_every_command(self):
        self.run_runner(["--config", "settings with spaces.toml"])


if __name__ == "__main__":
    unittest.main()
