"""Weekly Colima release watch.

Compares the latest abiosoft/colima release with LAST_TESTED (read from the
telima-core source, so there is no second copy) and opens one issue, deduped
by title, with a re-verify checklist.

Run: python3 scripts/colima_release.py [--dry-run]
Env: GITHUB_TOKEN (required unless --dry-run), GITHUB_REPOSITORY (owner/name).
"""
import argparse
import json
import os
import re
import sys
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
VERSION_RS = os.path.join(ROOT, "crates", "telima-core", "src", "version.rs")
LAST_TESTED_RE = re.compile(
    r"pub const LAST_TESTED: ColimaVersion = ColimaVersion::new\((\d+),\s*(\d+),\s*(\d+)\);"
)
TAG_RE = re.compile(r"^v?(\d+)\.(\d+)\.(\d+)$")
UPSTREAM = "abiosoft/colima"


def parse_last_tested(text):
    m = LAST_TESTED_RE.search(text)
    if not m:
        raise ValueError("LAST_TESTED not found in telima-core version.rs")
    return tuple(int(x) for x in m.groups())


def parse_tag(tag):
    m = TAG_RE.match(tag)
    if not m:
        raise ValueError("not a plain release tag: %r" % tag)
    return tuple(int(x) for x in m.groups())


def is_newer(latest, last_tested):
    return latest > last_tested


def format_version(version):
    return "%d.%d.%d" % version


def issue_title(latest):
    return "Colima v%s 검증 필요" % format_version(latest)


def issue_body(latest, last_tested):
    v = format_version(latest)
    last = format_version(last_tested)
    return "\n".join([
        "abiosoft/colima 최신 릴리스 **v%s**가 나왔습니다. 현재 검증 기준(LAST_TESTED)은 **%s**입니다." % (v, last),
        "",
        "## 재검증 체크리스트",
        "",
        "- [ ] `bash scripts/contract-local.sh` 실행, 전체 통과 확인 (telima-test-* 프로파일)",
        "- [ ] parse 픽스처(`colima status -j`, `colima ls -j`)가 v%s 출력과 맞는지 확인" % v,
        "- [ ] `crates/telima-core/src/version.rs`의 `LAST_TESTED`를 `ColimaVersion::new(%s)`로 올리기" % v.replace(".", ", "),
        "- [ ] 검증에 문제가 있으면 이 이슈에 내용을 남기고, 지원 범위(MIN_SUPPORTED) 판단 후 닫기",
        "",
        "_자동 생성: scripts/colima_release.py (주간 워크플로)_",
    ])


def decide(latest, last_tested, open_titles):
    """Return (action, reason). action is 'none', 'skip' or 'create'."""
    if not is_newer(latest, last_tested):
        return "none", "v%s is not newer than LAST_TESTED %s" % (
            format_version(latest), format_version(last_tested))
    title = issue_title(latest)
    if title in open_titles:
        return "skip", "open issue already exists: %s" % title
    return "create", "newer than LAST_TESTED and no open issue titled %s" % title


def _request(url, token=None, data=None):
    headers = {"Accept": "application/vnd.github+json", "User-Agent": "telima-colima-release-watch"}
    if token:
        headers["Authorization"] = "Bearer " + token
    body = None
    if data is not None:
        body = json.dumps(data).encode("utf-8")
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=body, headers=headers, method="POST" if body else "GET")
    with urllib.request.urlopen(req, timeout=30) as resp:
        return json.loads(resp.read().decode("utf-8"))


def fetch_latest_tag(token=None):
    data = _request("https://api.github.com/repos/%s/releases/latest" % UPSTREAM, token)
    return data["tag_name"]


def fetch_open_issue_titles(repo, token=None):
    titles = []
    page = 1
    while True:
        items = _request(
            "https://api.github.com/repos/%s/issues?state=open&per_page=100&page=%d" % (repo, page),
            token,
        )
        titles.extend(i["title"] for i in items if "pull_request" not in i)
        if len(items) < 100:
            return titles
        page += 1


def create_issue(repo, token, latest, last_tested):
    return _request(
        "https://api.github.com/repos/%s/issues" % repo,
        token,
        {"title": issue_title(latest), "body": issue_body(latest, last_tested)},
    )


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--dry-run", action="store_true", help="print the decision, create nothing")
    args = parser.parse_args(argv)

    with open(VERSION_RS, encoding="utf-8") as f:
        last_tested = parse_last_tested(f.read())
    token = os.environ.get("GITHUB_TOKEN")
    repo = os.environ.get("GITHUB_REPOSITORY", "")

    latest = parse_tag(fetch_latest_tag(token))
    if repo:
        open_titles = fetch_open_issue_titles(repo, token)
    else:
        open_titles = []
        print("GITHUB_REPOSITORY not set: dedupe check skipped", file=sys.stderr)
    action, reason = decide(latest, last_tested, open_titles)
    print("latest=%s last_tested=%s action=%s reason=%s" % (
        format_version(latest), format_version(last_tested), action, reason))

    if action == "create" and not args.dry_run:
        if not token or not repo:
            print("GITHUB_TOKEN and GITHUB_REPOSITORY are required to create an issue", file=sys.stderr)
            return 1
        issue = create_issue(repo, token, latest, last_tested)
        print("created issue #%s" % issue.get("number"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
