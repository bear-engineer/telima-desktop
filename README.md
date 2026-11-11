<p align="center"><img src="assets/icon/telima.png" width="128" alt="telima 아이콘: 컨테이너를 안은 픽셀 곰"></p>

# telima

macOS와 Linux에서 [Colima](https://github.com/abiosoft/colima) 프로필과 컨테이너·이미지·볼륨을 창과 트레이에서 다루는 데스크톱 앱입니다. Rust와 [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui)로 만들었습니다.

[English](README.en.md)

> 상태: v0.1.1. 초기 단계라 기능이 적고 바뀔 수 있습니다. 앱은 서명·공증하지 않습니다.

## 주요 기능

- Colima 프로필 목록, 만들기, 편집, 시작·중지·재시작, 삭제(데이터 디스크 함께 삭제 선택)
- 시작·중지·재시작 진행 단계와 로그, 경과 시간, 가동 시간 표시
- 컨테이너 목록(상태·포트·시작 시각), 시작·중지·재시작·삭제, 종료된 컨테이너 일괄 삭제
- 컨테이너 로그: 시각 표시, 따라가기, 검색
- 이미지·볼륨 목록과 미사용 항목 정리, Colima 캐시 정리
- 시작 시 Colima 버전 확인과 안내 배너
- 메뉴 막대(macOS)·시스템 트레이(Linux) 아이콘. 창을 닫아도 계속 실행됩니다
- 시스템 화면 모드(밝게·어둡게)를 따릅니다

자세한 내용은 [기능 명세](docs/features.md)와 [사용법 가이드](docs/guide.md)를 보세요.

## 설치

[GitHub Releases](https://github.com/bear-engineer/telima-desktop/releases)에서 받습니다. 릴리스마다 `SHA256SUMS-*.txt` 체크섬 파일이 함께 올라갑니다.

### macOS (Apple Silicon)

1. `telima-v0.1.1-macos-arm64.zip`을 받아 풀면 `telima.app`이 나옵니다. 응용 프로그램(`/Applications`)으로 옮깁니다.
2. 서명하지 않은 앱이라 처음 열 때 macOS가 막습니다. 다음 중 하나로 엽니다.
   - Finder에서 `telima.app`을 오른쪽 클릭(또는 Control 클릭) → 열기 → 열기. macOS 버전에 따라 이 방법이 막히면 시스템 설정 → 개인정보 보호 및 보안에서 "그래도 열기"를 누릅니다.
   - 터미널에서 격리 속성을 지웁니다.

     ```sh
     xattr -dr com.apple.quarantine /Applications/telima.app
     ```

macOS 13 이상이 필요합니다(`scripts/bundle-macos.sh`의 `LSMinimumSystemVersion`).

### Linux (x86_64)

`telima-v0.1.1-linux-x86_64.tar.gz`에는 실행 파일 `telima`, 데스크톱 항목 `io.github.bear_engineer.telima.desktop`, 아이콘 디렉터리 `hicolor/`, 이 README가 들어 있습니다.

```sh
tar -xzf telima-v0.1.1-linux-x86_64.tar.gz
cd telima-v0.1.1-linux-x86_64
install -Dm755 telima ~/.local/bin/telima
install -Dm644 io.github.bear_engineer.telima.desktop ~/.local/share/applications/io.github.bear_engineer.telima.desktop
mkdir -p ~/.local/share/icons
cp -R hicolor ~/.local/share/icons/
```

데스크톱 항목은 `Exec=telima`로 실행하므로 `~/.local/bin`이 `PATH`에 있어야 합니다. 트레이 아이콘은 StatusNotifierItem을 지원하는 패널에서만 보입니다. GNOME은 AppIndicator 확장이 필요합니다([docs/tray.md](docs/tray.md)).

## 요구 사항

- Colima 0.10.3 이상. 0.10.3에서 확인했습니다. 더 낮은 버전이면 "Colima 업데이트 필요", 더 높은 버전이면 "검증되지 않은 Colima 버전" 배너가 뜹니다(`crates/telima-core/src/version.rs`의 `MIN_SUPPORTED`, `LAST_TESTED`).
- 프로필 런타임은 docker만 지원합니다. 컨테이너·이미지·볼륨은 `~/.colima/<프로필>/docker.sock`으로 Docker Engine API에 직접 연결해 읽으므로 docker CLI는 없어도 됩니다.
- `colima`는 `PATH`, `/opt/homebrew/bin`, `/usr/local/bin`, `~/.local/bin`, `/home/linuxbrew/.linuxbrew/bin` 순서로 찾습니다.

## 소스에서 빌드

필요한 것:

- Rust 1.98.1 (`rust-toolchain.toml`에 고정. rustup이 설치합니다)
- macOS: Command Line Tools (`xcode-select --install`). Xcode 전체는 필요 없습니다.
- Linux (Debian/Ubuntu): `sudo apt install clang libxkbcommon-x11-dev libwayland-dev libx11-xcb-dev libfontconfig-dev libvulkan1 libssl-dev libdbus-1-dev pkg-config`
- 앱을 실행하려면 Colima (빌드에는 필요 없음)

```sh
cargo run -p telima
cargo test --workspace
```

패키징:

- macOS: `scripts/bundle-macos.sh`가 아이콘을 넣은 서명하지 않은 `target/release/bundle/telima.app`을 만듭니다. `cargo run`으로 실행하면 Dock에 기본 아이콘이 보입니다.
- Linux: `packaging/linux/io.github.bear_engineer.telima.desktop`을 `~/.local/share/applications/`에, `assets/icon/hicolor/`의 내용을 `~/.local/share/icons/hicolor/`에 복사합니다. 창이 같은 앱 ID(`crates/telima-ui/src/lib.rs`의 `APP_ID`)를 알리므로 데스크톱 환경이 아이콘을 보여 줍니다.
- 아이콘: 원본은 `assets/icon/telima.png`(1024 px) 하나입니다. `scripts/icons.sh`(macOS)가 여러 크기와 `telima.icns`를 다시 만듭니다. 출처는 [docs/icon.md](docs/icon.md)에 있습니다.
- 릴리스: `v*` 태그를 올리면 `.github/workflows/release.yml`이 macOS zip과 Linux tar.gz를 만들고 [CHANGELOG.md](CHANGELOG.md)의 해당 버전 절을 릴리스 노트로 씁니다.

## 구조

의존성은 안쪽으로만 향합니다. CI에서 `scripts/check_layers.py`가 검사합니다.

| 크레이트 | 역할 |
|---|---|
| `telima-core` | 도메인 엔터티, 규칙, 포트 트레이트 |
| `telima-app` | 유스케이스, 상태, 뷰 모델, 한국어 문구 |
| `telima-colima` | Colima CLI 어댑터 |
| `telima-docker` | Docker Engine API 어댑터 (bollard) |
| `telima-tray` | 메뉴 막대·시스템 트레이 어댑터 |
| `telima-ui` | GPUI 화면 |
| `telima` | 실행 파일. 전체를 연결합니다 |

## 문서

- [docs/features.md](docs/features.md): 기능 명세
- [docs/guide.md](docs/guide.md): 사용법 가이드
- [docs/tray.md](docs/tray.md): 트레이 아이콘
- [docs/dev.md](docs/dev.md): 개발용 환경 변수
- [CHANGELOG.md](CHANGELOG.md): 변경 기록
- [CONTRIBUTING.md](CONTRIBUTING.md): 기여 안내

## 라이선스

[Apache License 2.0](LICENSE-APACHE) 또는 [MIT](LICENSE-MIT) 중 하나를 골라 쓸 수 있습니다.
