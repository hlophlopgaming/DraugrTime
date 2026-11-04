"""M0 acceptance checks; run with python3 -m unittest discover -s tests."""
import json
from pathlib import Path
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]


class SkeletonTests(unittest.TestCase):
    def test_workspace_contains_all_four_crates(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
        self.assertEqual(set(manifest["workspace"]["members"]),
                         {"core", "watchers", "daemon", "app"})
        for member in manifest["workspace"]["members"]:
            self.assertTrue((ROOT / member / "Cargo.toml").is_file())

    def test_desktop_is_a_single_native_draugr_time_binary(self):
        manifest = tomllib.loads((ROOT / 'app/Cargo.toml').read_text())
        self.assertEqual(manifest['package']['name'], 'draugr-time')
        self.assertIn('iced', manifest['dependencies'])
        self.assertNotIn('tauri', manifest['dependencies'])
        self.assertFalse((ROOT / 'app/tauri.conf.json').exists())
        self.assertFalse((ROOT / 'app/frontend').exists())
        self.assertIn('winresource', (ROOT / 'app/build.rs').read_text())
        lock = tomllib.loads((ROOT / 'Cargo.lock').read_text())
        self.assertFalse(any(package['name'].startswith(('tauri', 'wry', 'webkit')) for package in lock['package']))
        main = (ROOT / 'app/src/main.rs').read_text()
        self.assertIn('desktop::run()', main)

    def test_ci_checks_and_builds_on_three_platforms(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        for required in ("ubuntu-22.04", "windows-latest", "macos-15",
                         "cargo fmt --check", "cargo clippy --all-targets -- -D warnings",
                         "cargo test", "cargo build --package draugr-time --release"):
            self.assertIn(required, workflow)
        for removed in ("npm ", "WebKit", "dagdatime-app", "dagdatime-iced"):
            self.assertNotIn(removed, workflow)

class WaylandPackageTests(unittest.TestCase):
    def test_kde_declarative_entrypoint_uses_required_ui_directory(self):
        package = ROOT / 'watchers/wayland/kde'
        metadata = json.loads((package / 'metadata.json').read_text())
        self.assertEqual(metadata['X-Plasma-MainScript'], 'ui/main.qml')
        self.assertTrue((package / 'contents/ui/main.qml').is_file())
