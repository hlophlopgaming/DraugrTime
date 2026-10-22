import importlib.util
import os
from pathlib import Path
import plistlib
import struct
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]

class DistributionTests(unittest.TestCase):
    def test_native_icons_wrap_the_existing_png(self):
        png = (ROOT / 'logo/appIcon128.png').read_bytes()
        ico = (ROOT / 'logo/draugr-time.ico').read_bytes()
        self.assertEqual(struct.unpack('<HHH', ico[:6]), (0, 1, 1))
        self.assertEqual(ico[22:], png)
        icns = (ROOT / 'logo/draugr-time.icns').read_bytes()
        self.assertEqual(icns[:4], b'icns')
        self.assertEqual(struct.unpack('>I', icns[4:8])[0], len(icns))
        self.assertEqual(icns[16:], png)

    def test_mac_archive_contains_launchable_bundle_metadata_and_executable(self):
        spec = importlib.util.spec_from_file_location('bundle', ROOT / 'scripts/package_macos.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            source = folder / 'draugr-time'
            source.write_bytes(b'fake executable')
            destination = folder / 'Draugr Time.app'
            module.create_bundle(source, destination, '1.0.0')
            contents = destination / 'Contents'
            info = plistlib.loads((contents / 'Info.plist').read_bytes())
            self.assertEqual(info['CFBundleIdentifier'], 'org.draugrtime.app')
            self.assertEqual(info['CFBundleShortVersionString'], '1.0.0')
            self.assertEqual((contents / 'MacOS' / info['CFBundleExecutable']).read_bytes(), source.read_bytes())
            self.assertTrue((contents / 'Resources' / info['CFBundleIconFile']).is_file())

    def test_windows_installer_ships_cli_and_adds_it_to_user_path(self):
        script = (ROOT / 'packaging/windows/installer.iss').read_text()
        for required in ('draugr-time.exe', 'draugr-time-cli.exe',
                         'PrivilegesRequired=lowest', 'HKCU', 'Environment',
                         'ValueName: "Path"', 'NeedsAddPath'):
            self.assertIn(required, script)

    def test_appimage_appdir_has_apprun_desktop_and_icon(self):
        spec = importlib.util.spec_from_file_location('appimage', ROOT / 'scripts/package_appimage.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            source = folder / 'draugr-time'
            source.write_bytes(b'fake executable')
            appdir = folder / 'AppDir'
            module.assemble_appdir(source, appdir)
            self.assertTrue((appdir / 'usr/bin/draugr-time').is_file())
            self.assertTrue((appdir / 'AppRun').read_text().startswith('#!/bin/sh'))
            if os.name != 'nt':
                self.assertTrue((appdir / 'AppRun').stat().st_mode & 0o111)
            self.assertTrue((appdir / 'org.draugrtime.app.desktop').is_file())
            self.assertTrue((appdir / 'org.draugrtime.app.png').is_file())
            self.assertTrue((appdir / 'usr/share/applications/org.draugrtime.app.desktop').is_file())
            self.assertTrue((appdir / 'usr/share/icons/hicolor/128x128/apps/org.draugrtime.app.png').is_file())

    def test_macos_dmg_stages_app_and_applications_link(self):
        script = (ROOT / 'scripts/package_dmg.sh').read_text()
        for required in ('hdiutil', 'Draugr Time.app', '/Applications'):
            self.assertIn(required, script)

    def test_ci_builds_and_attaches_installers(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        for required in ('Build Windows installer', 'Build Linux AppImage',
                         'Assemble macOS DMG', 'Draugr-Time-Setup.exe',
                         'Draugr-Time-x86_64.AppImage', 'Draugr-Time-macOS.dmg'):
            self.assertIn(required, workflow)

    def test_linux_release_archive_uses_the_app_icon(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn(
            'cp logo/appIcon128.png artifacts/Draugr-Time-Linux/icons/hicolor/128x128/apps/org.draugrtime.app.png',
            workflow,
        )
        self.assertNotIn('cp logo/blackDraugr128.png', workflow)
