"""Dependency-direction guard. Single source of truth for which crate may depend on what.

Run: python3 scripts/check_layers.py   (reads `cargo metadata`; exit 1 on violation)
"""
import json
import subprocess
import sys

# Crates and libraries each layer must NOT depend on (normal/build deps; dev-deps ignored).
FRAMEWORKS = {"gpui-kit", "tokio", "bollard"}
FORBIDDEN = {
    "telima-core": FRAMEWORKS | {"telima-app", "telima-colima", "telima-docker", "telima-ui", "telima"},
    "telima-app": FRAMEWORKS | {"telima-colima", "telima-docker", "telima-ui", "telima"},
    "telima-colima": {"gpui-kit", "bollard", "telima-app", "telima-docker", "telima-ui", "telima"},
    "telima-docker": {"gpui-kit", "telima-app", "telima-colima", "telima-ui", "telima"},
    "telima-ui": {"tokio", "bollard", "telima-colima", "telima-docker", "telima"},
}


def violations(metadata):
    members = set(metadata["workspace_members"])
    found = []
    for pkg in metadata["packages"]:
        if pkg["id"] not in members:
            continue
        banned = FORBIDDEN.get(pkg["name"], set())
        for dep in pkg["dependencies"]:
            if dep.get("kind") == "dev":
                continue
            if dep["name"] in banned:
                found.append(f'{pkg["name"]} -> {dep["name"]}')
    return found


def main():
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        check=True, capture_output=True, text=True,
    ).stdout
    found = violations(json.loads(out))
    for v in found:
        print(f"layer violation: {v}")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
