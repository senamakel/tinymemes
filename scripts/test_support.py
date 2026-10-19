"""Shared subprocess helpers for platform-independent Python regression tests."""
from pathlib import Path
import os
import shutil
import subprocess


def git_bash_executable(*, path=None, windows=None):
    """Select Git for Windows Bash explicitly instead of a possible WSL shim."""
    windows = os.name == "nt" if windows is None else windows
    if not windows:
        executable = shutil.which("bash", path=path)
        if executable:
            return executable
        raise FileNotFoundError("Bash executable was not found")

    git = shutil.which("git", path=path)
    if git:
        for parent in Path(git).absolute().parents:
            if parent.name.casefold() == "git":
                candidate = parent / "bin" / "bash.exe"
                if candidate.is_file():
                    return str(candidate)
    raise FileNotFoundError("Git for Windows Bash was not found beside the Git executable")


def bash_command(script, *args, path=None, windows=None):
    """Build a Bash command with a checkout-relative script path."""
    script_path = Path(script)
    if script_path.is_absolute():
        raise ValueError("Bash test scripts must be passed as checkout-relative paths")
    return [git_bash_executable(path=path, windows=windows), script_path.as_posix(), *map(str, args)]


def run_bash(script, *args, cwd, env=None, path=None, windows=None):
    """Run a checkout-relative script through the selected Bash executable."""
    command = bash_command(script, *args, path=path, windows=windows)
    executable = command[0]
    version = subprocess.run(
        [executable, "--version"],
        cwd=cwd,
        text=True,
        capture_output=True,
        check=False,
    )
    version_text = (version.stdout or version.stderr).splitlines()
    version_text = version_text[0] if version_text else f"version command exited {version.returncode}"
    result = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    result.bash_executable = executable
    result.bash_version = version_text
    return result


def bash_diagnostic(result):
    """Summarize a Bash run without dumping its environment."""
    return (
        f"bash executable: {result.bash_executable}\n"
        f"bash version: {result.bash_version}\n"
        f"stdout: {result.stdout!r}\n"
        f"stderr: {result.stderr!r}"
    )
