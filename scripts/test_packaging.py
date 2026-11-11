"""Packaging stays consistent with the one app id defined in telima-ui (SOT)."""

import re
import struct
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HICOLOR_SIZES = (16, 32, 48, 64, 128, 256, 512)
TRAY = ROOT / "assets/icon/tray"
TRAY_STATES = ("stopped", "running", "busy", "error")
TRAY_LINUX_SIZES = (22, 24)
# light = dark glyph for light panels, dark = light glyph for dark panels
TRAY_LINUX_PANELS = ("light", "dark")


def app_id():
    source = (ROOT / "crates/telima-ui/src/lib.rs").read_text(encoding="utf-8")
    match = re.search(r'pub const APP_ID: &str = "([^"]+)";', source)
    if match is None:
        raise AssertionError("APP_ID is not defined in crates/telima-ui/src/lib.rs")
    return match.group(1)


def png_size(path):
    data = path.read_bytes()[:24]
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise AssertionError(f"{path} is not a PNG")
    return struct.unpack(">II", data[16:24])


def desktop_entry(path):
    entries = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" in line and not line.startswith("#"):
            key, value = line.split("=", 1)
            entries[key.strip()] = value.strip()
    return entries


class PackagingTest(unittest.TestCase):
    def test_app_id_is_reverse_dns(self):
        self.assertRegex(app_id(), r"^[a-z][a-z0-9_]*(\.[a-z0-9_]+){2,}$")

    def test_master_icon_is_1024_png(self):
        self.assertEqual(png_size(ROOT / "assets/icon/telima.png"), (1024, 1024))

    def test_macos_icns_exists(self):
        icns = ROOT / "assets/icon/telima.icns"
        self.assertEqual(icns.read_bytes()[:4], b"icns")

    def test_hicolor_icons_are_named_after_app_id(self):
        for size in HICOLOR_SIZES:
            path = ROOT / f"assets/icon/hicolor/{size}x{size}/apps/{app_id()}.png"
            self.assertEqual(png_size(path), (size, size), path)

    def test_desktop_entry_matches_app_id(self):
        path = ROOT / f"packaging/linux/{app_id()}.desktop"
        entry = desktop_entry(path)
        self.assertEqual(entry["Icon"], app_id())
        self.assertEqual(entry["StartupWMClass"], app_id())
        self.assertEqual(entry["Exec"], "telima")
        self.assertEqual(entry["Type"], "Application")

    def test_macos_tray_templates_are_18pt_at_1x_and_2x(self):
        for state in TRAY_STATES:
            self.assertEqual(png_size(TRAY / f"macos/{state}.png"), (18, 18), state)
            self.assertEqual(png_size(TRAY / f"macos/{state}@2x.png"), (36, 36), state)

    def test_linux_tray_icons_are_22_and_24_px_for_both_panels(self):
        for size in TRAY_LINUX_SIZES:
            for panel in TRAY_LINUX_PANELS:
                for state in TRAY_STATES:
                    path = TRAY / f"linux/{size}/{state}-{panel}.png"
                    self.assertEqual(png_size(path), (size, size), path)

    def test_icons_script_generates_tray_icons(self):
        script = (ROOT / "scripts/icons.sh").read_text(encoding="utf-8")
        self.assertIn("assets/icon/tray", script)

    def test_bundle_script_reads_app_id_instead_of_repeating_it(self):
        script = (ROOT / "scripts/bundle-macos.sh").read_text(encoding="utf-8")
        self.assertNotIn(app_id(), script)
        self.assertIn("APP_ID", script)


if __name__ == "__main__":
    unittest.main()
