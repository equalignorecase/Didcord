"""Install the real release payload without launching it; disposable Windows runner only."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

from test_installer import check_shortcut, wait_for


def main():
    if os.name != "nt":
        raise SystemExit("Requires a disposable Windows runner")
    import ctypes
    import winreg

    setup, payload = (Path(value).resolve() for value in sys.argv[1:3])
    version = sys.argv[3]
    programs = ctypes.create_unicode_buffer(1024)
    ctypes.OleDLL("shell32").SHGetFolderPathW(None, 2, None, 0, programs)
    shortcut = Path(programs.value) / "Didcord.lnk"
    key = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Didcord"
    assert not shortcut.exists(), "Refusing to overwrite an existing shortcut"
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key):
            raise AssertionError("Refusing to overwrite an existing installation")
    except FileNotFoundError:
        pass
    with tempfile.TemporaryDirectory(prefix="didcord-release-install-") as temporary:
        root = Path(temporary)
        installed = root / "Didcord žluťoučký"
        try:
            subprocess.run(f'"{setup}" /S /D={installed}', check=True, timeout=120)
            expected = {p.relative_to(payload) for p in payload.rglob("*") if p.is_file()
                        and p.name not in ("setup.ps1", "install-notifications.ps1")}
            actual = {p.relative_to(installed) for p in installed.rglob("*") if p.is_file()}
            assert actual == expected | {Path("uninstall.exe")}, (expected - actual, actual - expected)
            for path in expected:
                assert hashlib.sha256((payload / path).read_bytes()).digest() == hashlib.sha256((installed / path).read_bytes()).digest(), path
            check_shortcut(shortcut, installed / "didcord.exe")
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key, 0, winreg.KEY_READ | winreg.KEY_WOW64_64KEY) as registry:
                assert winreg.QueryValueEx(registry, "DisplayVersion")[0] == version
        finally:
            uninstaller = installed / "uninstall.exe"
            if uninstaller.exists():
                copy = root / "uninstall-test.exe"
                shutil.copy2(uninstaller, copy)
                subprocess.run(f'"{copy}" /S _?={installed}', check=True, timeout=60)
                def remove_copy():
                    try:
                        copy.unlink(missing_ok=True)
                        return True
                    except PermissionError:
                        return False
                wait_for(remove_copy)
        assert not installed.exists()
        assert not shortcut.exists()
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key):
                raise AssertionError("Uninstall registry entry remains")
        except FileNotFoundError:
            pass
    print("Actual release installer passed: payload hashes, shortcut, version and complete uninstall; app never launched.")


if __name__ == "__main__":
    main()
