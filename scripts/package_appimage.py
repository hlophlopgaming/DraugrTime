"""Assemble the Linux AppImage AppDir and, when appimagetool is available, build the AppImage."""
from pathlib import Path
import os
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]

APPRUN = """#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export PATH="$HERE/usr/bin:$PATH"
exec "$HERE/usr/bin/draugr-time" "$@"
"""


def assemble_appdir(executable: Path, appdir: Path) -> None:
    """Populate an AppDir with the desktop binary, AppRun, desktop entry and icon."""
    (appdir / "usr/bin").mkdir(parents=True, exist_ok=True)
    (appdir / "usr/share/applications").mkdir(parents=True, exist_ok=True)
    (appdir / "usr/share/icons/hicolor/128x128/apps").mkdir(parents=True, exist_ok=True)
    shutil.copy2(executable, appdir / "usr/bin/draugr-time")
    (appdir / "AppRun").write_text(APPRUN)
    (appdir / "AppRun").chmod(0o755)
    desktop = ROOT / "packaging/org.draugrtime.app.desktop"
    shutil.copy2(desktop, appdir / "org.draugrtime.app.desktop")
    shutil.copy2(desktop, appdir / "usr/share/applications/org.draugrtime.app.desktop")
    icon = ROOT / "logo/appIcon128.png"
    shutil.copy2(icon, appdir / "org.draugrtime.app.png")
    shutil.copy2(icon, appdir / "usr/share/icons/hicolor/128x128/apps/org.draugrtime.app.png")


def main() -> int:
    release = ROOT / "target/release"
    executable = release / "draugr-time"
    if not executable.is_file():
        print("target/release/draugr-time not found; build it first", file=sys.stderr)
        return 1
    appdir = ROOT / "target/appimage/AppDir"
    shutil.rmtree(appdir, ignore_errors=True)
    assemble_appdir(executable, appdir)

    tool = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "target/appimagetool"
    output = release / "Draugr-Time-x86_64.AppImage"
    env = dict(os.environ, APPIMAGE_EXTRACT_AND_RUN="1")
    subprocess.run([str(tool), str(appdir), str(output)], env=env, check=True)
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
