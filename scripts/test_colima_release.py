"""Tests for the weekly Colima release watch: pure parts only (no network)."""
import unittest

from colima_release import (
    decide,
    format_version,
    is_newer,
    issue_body,
    issue_title,
    parse_last_tested,
    parse_tag,
)

VERSION_RS = """
/// Oldest Colima release the app works with.
pub const MIN_SUPPORTED: ColimaVersion = ColimaVersion::new(0, 10, 3);

/// Newest Colima release the contract tests were run against.
pub const LAST_TESTED: ColimaVersion = ColimaVersion::new(0, 10, 3);
"""


class ParseLastTestedTest(unittest.TestCase):
    def test_reads_last_tested_not_min_supported(self):
        self.assertEqual(parse_last_tested(VERSION_RS), (0, 10, 3))

    def test_reads_a_bumped_value(self):
        text = VERSION_RS.replace(
            "LAST_TESTED: ColimaVersion = ColimaVersion::new(0, 10, 3)",
            "LAST_TESTED: ColimaVersion = ColimaVersion::new(0, 11, 2)",
        )
        self.assertEqual(parse_last_tested(text), (0, 11, 2))

    def test_missing_constant_is_an_error(self):
        with self.assertRaises(ValueError):
            parse_last_tested("pub const MIN_SUPPORTED: ColimaVersion = ColimaVersion::new(0, 10, 3);")


class ParseTagTest(unittest.TestCase):
    def test_accepts_v_prefix(self):
        self.assertEqual(parse_tag("v0.11.0"), (0, 11, 0))

    def test_accepts_bare_version(self):
        self.assertEqual(parse_tag("0.10.10"), (0, 10, 10))

    def test_rejects_non_release_tags(self):
        for bad in ["nightly", "v0.11", "v0.11.0-rc1", ""]:
            with self.subTest(tag=bad):
                with self.assertRaises(ValueError):
                    parse_tag(bad)


class CompareTest(unittest.TestCase):
    def test_newer_minor(self):
        self.assertTrue(is_newer((0, 11, 0), (0, 10, 3)))

    def test_same_version_is_not_newer(self):
        self.assertFalse(is_newer((0, 10, 3), (0, 10, 3)))

    def test_older_is_not_newer(self):
        self.assertFalse(is_newer((0, 10, 2), (0, 10, 3)))

    def test_numeric_not_lexical(self):
        self.assertTrue(is_newer((0, 10, 10), (0, 10, 9)))


class TitleTest(unittest.TestCase):
    def test_format_version(self):
        self.assertEqual(format_version((0, 11, 0)), "0.11.0")

    def test_title(self):
        self.assertEqual(issue_title((0, 11, 0)), "Colima v0.11.0 검증 필요")


class DecideTest(unittest.TestCase):
    def test_not_newer_does_nothing(self):
        action, _ = decide((0, 10, 3), (0, 10, 3), [])
        self.assertEqual(action, "none")

    def test_newer_without_open_issue_creates(self):
        action, _ = decide((0, 11, 0), (0, 10, 3), ["Something else"])
        self.assertEqual(action, "create")

    def test_newer_with_open_issue_with_same_title_skips(self):
        action, _ = decide((0, 11, 0), (0, 10, 3), [issue_title((0, 11, 0)), "x"])
        self.assertEqual(action, "skip")

    def test_other_version_issue_does_not_block(self):
        action, _ = decide((0, 11, 0), (0, 10, 3), [issue_title((0, 10, 9))])
        self.assertEqual(action, "create")


class BodyTest(unittest.TestCase):
    def test_checklist_names_the_steps(self):
        body = issue_body((0, 11, 0), (0, 10, 3))
        self.assertIn("bash scripts/contract-local.sh", body)
        self.assertIn("parse", body)
        self.assertIn("LAST_TESTED", body)
        self.assertIn("0.10.3", body)
        self.assertIn("0.11.0", body)
        self.assertIn("- [ ]", body)


if __name__ == "__main__":
    unittest.main()
