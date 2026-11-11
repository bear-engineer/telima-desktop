"""Tests for the dependency-direction guard (Clean Architecture: dependencies point inward)."""
import unittest

from check_layers import violations


def meta(deps):
    """Build a minimal `cargo metadata` document: {crate: [dep names]}."""
    return {
        "packages": [
            {"name": n, "id": n, "dependencies": [{"name": d, "kind": None} for d in ds]}
            for n, ds in deps.items()
        ],
        "workspace_members": list(deps),
    }


class LayerGuardTest(unittest.TestCase):
    def test_clean_layering_passes(self):
        m = meta({
            "telima-core": ["thiserror"],
            "telima-app": ["telima-core"],
            "telima-colima": ["telima-core", "tokio"],
            "telima-docker": ["telima-core", "bollard"],
            "telima-ui": ["telima-app", "gpui-kit"],
            "telima": ["telima-ui", "telima-colima", "telima-docker", "tokio"],
        })
        self.assertEqual(violations(m), [])

    def test_core_must_not_know_frameworks(self):
        m = meta({"telima-core": ["gpui-kit", "tokio"]})
        self.assertEqual(sorted(violations(m)), ["telima-core -> gpui-kit", "telima-core -> tokio"])

    def test_app_must_not_know_adapters_or_ui(self):
        m = meta({"telima-app": ["telima-core", "telima-colima", "bollard", "gpui-kit"]})
        self.assertEqual(
            sorted(violations(m)),
            ["telima-app -> bollard", "telima-app -> gpui-kit", "telima-app -> telima-colima"],
        )

    def test_ui_must_not_reach_adapters(self):
        m = meta({"telima-ui": ["telima-app", "telima-docker", "bollard"]})
        self.assertEqual(sorted(violations(m)), ["telima-ui -> bollard", "telima-ui -> telima-docker"])

    def test_adapters_must_not_know_each_other_or_ui(self):
        m = meta({"telima-colima": ["telima-core", "telima-docker", "gpui-kit"]})
        self.assertEqual(sorted(violations(m)), ["telima-colima -> gpui-kit", "telima-colima -> telima-docker"])

    def test_dev_dependencies_are_ignored(self):
        m = meta({"telima-core": []})
        m["packages"][0]["dependencies"].append({"name": "tokio", "kind": "dev"})
        self.assertEqual(violations(m), [])


if __name__ == "__main__":
    unittest.main()
