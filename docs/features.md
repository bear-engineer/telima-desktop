# 기능 명세 (v0.1.0)

telima가 하는 일과 그때 실제로 실행하는 명령을 영역별로 적는다. 따옴표 안의 문구는 앱 화면에 나오는 그대로다. 사용 순서는 [사용법 가이드](guide.md)를 본다.

## 공통

- Colima는 CLI로 다룬다. `colima`는 `PATH`, `/opt/homebrew/bin`, `/usr/local/bin`, `~/.local/bin`, `/home/linuxbrew/.linuxbrew/bin` 순서로 찾는다.
  근거: `crates/telima-colima/src/locate.rs`
- 컨테이너·이미지·볼륨은 선택한 프로필의 `~/.colima/<프로필>/docker.sock`으로 Docker Engine API(bollard)를 직접 호출한다. docker CLI는 쓰지 않는다. 요청 제한 시간은 120초다.
  근거: `crates/telima/src/main.rs`, `crates/telima-docker/src/engine.rs`
- 지원 OS는 macOS와 Linux다. 다른 OS에서는 빌드되지 않는다.
- 화면 문구는 한국어뿐이다.

## 프로필

### 목록

- 왼쪽 "프로필" 목록과 툴바의 프로필 드롭다운에 모든 프로필을 상태와 함께 보인다. 상태는 "실행 중", "중지", "처리 중", "오류" 넷이다.
- 목록은 `colima ls -j`로 읽는다. 10초마다 다시 읽고, "새로고침"을 누르면 바로 읽는다.
  근거: `crates/telima-app/src/runtime.rs`의 `PROFILE_POLL_INTERVAL`
- 프로필이 없으면 "프로필이 없습니다."와 "새 프로필" 버튼을 보인다.
- 선택한 프로필의 "구성" 영역에 "런타임", "VM 종류", "마운트", "아키텍처", "CPU", "메모리", "디스크", "IP 주소", "가동 시간", "Kubernetes", "디렉터리 마운트", "docker 소켓"을 보인다.

### 가동 시간과 버전

- 실행 중인 프로필은 `colima status -p <프로필> --json`과 `colima version -p <프로필>`을 실행해 Colima·Docker 버전을 읽는다.
- 가동 시간은 `~/.colima/_lima/<인스턴스>/ha.pid` 파일의 수정 시각으로 계산한다. Colima 0.10.3에는 가동 시간 값이 없기 때문이다. 표시는 "가동 3시간 12분"처럼 하고 1분이 안 되면 "1분 미만"이다. 파일을 읽지 못하면 "—"다.
  근거: `crates/telima-colima/src/cli.rs`의 `runtime_info`, `crates/telima-app/src/view/time_text.rs`

### 만들기

- "새 프로필"(툴바), "새 프로필…"(프로필 드롭다운 맨 아래), 트레이 메뉴에서 연다.
- 입력 항목: "이름", "CPU", "메모리", "디스크", "VM 종류", "마운트", "Rosetta", "IP 주소", "디렉터리", "Kubernetes".
- 런타임은 docker로 고정이다. containerd, incus는 지원하지 않는다.
- 규칙(`crates/telima-core/src/rules.rs`, `crates/telima-core/src/profile.rs`):
  - 이름: 영문 소문자, 숫자, `-`만. 최대 32자. `-`로 시작할 수 없다.
  - CPU·메모리·디스크는 1 이상.
  - VM 종류: macOS는 vz, qemu, krunkit. Linux는 qemu만.
  - 마운트 방식: vz·krunkit은 virtiofs, sshfs, 9p. qemu는 sshfs, 9p.
  - Rosetta는 macOS의 vz VM에서만 켤 수 있다.
  - Kubernetes는 docker 런타임에서만 켤 수 있다.
  - 디렉터리는 `/`로 시작하는 절대 경로여야 하고 같은 경로를 두 번 넣을 수 없다. 끝의 `/`는 무시하고 비교한다. 디렉터리마다 "읽기·쓰기"와 "읽기 전용"을 고른다. 하나도 없으면 Colima 기본값(홈 디렉터리)을 쓴다.
- "만들기"를 누르면 다음을 실행한다. 만들기는 곧 첫 시작이므로 시작 진행 화면이 이어진다.

  ```
  colima start <이름> --cpus N --memory N --disk N --vm-type <종류> --mount-type <방식> --runtime docker
        [--vz-rosetta] [--network-address] [--mount <경로>[:w] ...] [--kubernetes]
  ```

  근거: `crates/telima-colima/src/cli.rs`의 `create_args`

### 편집

- 프로필의 "편집" 버튼으로 연다. 같은 폼을 쓰고 버튼이 "저장"이다.
- "이름", "VM 종류", 런타임은 만든 뒤 바꿀 수 없다("만든 뒤에는 바꿀 수 없습니다"). 디스크는 늘리기만 할 수 있다.
- 저장은 Colima 명령을 실행하지 않고 프로필의 `colima.yaml`에서 `cpu`, `memory`, `disk`, `mounts`, `kubernetes` 항목을 고쳐 쓴다. 다른 줄과 주석은 그대로 둔다.
  근거: `crates/telima-colima/src/cli.rs`의 `update`
- 재시작 규칙: CPU, 메모리, 디스크, 마운트 방식, Rosetta, 네트워크 주소, 마운트, Kubernetes는 VM이 다시 시작할 때 적용된다. 실행 중인 프로필에서 이 값을 바꾸면 폼 아래에 "… 변경은 프로필을 재시작해야 적용됩니다."가 나온다. 중지된 프로필은 다음 시작 때 적용된다. 앱이 자동으로 재시작하지는 않는다.
  근거: `crates/telima-core/src/rules.rs`의 `RESTART_FIELDS`, `crates/telima-app/src/view/daily.rs`의 `restart_required_text`

### 시작·중지·재시작

- 툴바, 트레이 메뉴에서 실행한다. 툴바는 상태에 맞는 버튼만 보인다.

  | 상태 | 가능한 동작 |
  |---|---|
  | 실행 중 | 재시작, 중지 |
  | 중지 | 시작 |
  | 오류 | 시작, 재시작, 중지 (복구용) |
  | 처리 중 | 없음 (편집·삭제도 막힘) |

  근거: `crates/telima-app/src/view/mod.rs`의 `profile_actions`
- 실행 명령: `colima start <프로필>`, `colima stop <프로필>`, `colima restart <프로필>`. 실행 전에 `colima ls -j`로 프로필이 있는지 확인한다.
- 같은 프로필에 다른 작업이 진행 중이면 "다른 작업이 진행 중입니다. 끝난 뒤 다시 시도해 주세요."를 보인다.

### 진행 단계와 로그

- 실행 중에는 툴바와 진행 화면에 "시작 중", "중지 중", "재시작 중"을, 끝나면 "실행 중", "중지됨", "실패", "취소됨"을 보인다. 경과 시간은 "01:23 경과"처럼 보인다.
- 시작·재시작은 "시작 단계" 6단계를 보인다: "설정 확인", "네트워크", "VM 부팅", "프로비저닝", "docker 시작", "상태 확인". 단계는 Colima 로그의 낱말(config, network, boot, provision, docker, health 등)로 추정한다. 첫 단계가 나오기 전에는 "준비 중"이다.
  근거: `crates/telima-app/src/view/start_steps.rs`
- 로그는 "시작 로그", "재시작 로그", "중지 로그" 제목 아래 `INFO[0012]`처럼 수준과 경과 초를 붙여 보인다. 수준은 INFO, WARN, ERRO다. 로그가 오기 전에는 "로그를 기다리는 중…"이다.
- 시작 중에는 "시작 취소"가 있다. 누르면 명령을 중단한다. 안내: "취소하면 VM을 멈춥니다. 만든 디스크는 남습니다."
- 진행 화면은 "닫기"로 닫는다. 프로필마다 마지막 작업의 로그를 기억한다.

### 삭제

- "구성" 영역 아래 "프로필 삭제"에서 연다. 처리 중인 프로필은 삭제할 수 없다.
- "데이터 디스크도 삭제" 스위치(기본 꺼짐): "컨테이너 이미지와 볼륨이 담긴 디스크입니다."
  - 끄면: VM과 컨테이너가 지워지고 이미지·볼륨이 든 데이터 디스크는 남는다. "같은 이름으로 다시 만들면 이미지와 볼륨을 이어서 쓸 수 있습니다."
  - 켜면: VM 디스크, 컨테이너, 볼륨, 이미지가 모두 지워진다. 버튼에 지울 볼륨 수와 크기를 보인다(예: "볼륨 3개, 1.2GB 삭제").
- 확인하려면 프로필 이름을 그대로 입력해야 버튼이 켜진다("확인하려면 '<이름>'을(를) 입력하세요").
- 실행 명령: `colima delete -f <프로필>`, 데이터까지 지울 때 `colima delete -f --data <프로필>`.
  근거: `crates/telima-colima/src/cli.rs`의 `delete_args`, `crates/telima-app/src/view/mod.rs`의 `confirm_delete_profile`

## 컨테이너

- "컨테이너" 탭. 프로필이 실행 중일 때만 보인다. 중지된 프로필은 "프로필이 중지되어 있습니다. 시작하면 컨테이너·이미지·볼륨을 볼 수 있습니다."를 보인다.
- 중지된 것을 포함해 모든 컨테이너를 읽는다(`all=true`). 시작 시각은 컨테이너마다 inspect로 읽고, 실패하면 비워 둔다.
- 열: "이름", "ID", "이미지", "상태", "포트", "시작". ID 열은 창이 좁으면 숨는다.
  - 상태: "실행 중 · 2시간", "종료됨 · 코드 0", "오류 · 종료 코드 1", "재시작 중 · 종료 코드 N" 등.
  - 포트: `8080→80/tcp` 형식. 여럿이면 첫 매핑과 `+N`을 보이고 마우스를 올리면 전체를 보인다.
- 위쪽: "이름·이미지로 필터" 입력, "전체"·"실행"·"종료" 상태 필터(개수 표시), 새로고침 버튼, 종료된 컨테이너가 있으면 "종료된 N개 삭제".
- 행 동작: 시작·중지 버튼, 더보기 메뉴의 "로그 보기", "재시작", "삭제".
- Docker API 동작:
  - 시작 `start_container`, 재시작 `restart_container`.
  - 중지 `stop_container`, 10초 안에 끝나지 않으면 엔진이 강제 종료한다.
  - 삭제 `remove_container`. 실행 중인 컨테이너는 강제(force) 삭제하며 확인 창이 그 사실을 알린다.
  - "종료된 N개 삭제"는 종료된 컨테이너만 하나씩 지운다(강제 아님). 실행 중인 컨테이너와 볼륨은 그대로 둔다.
  근거: `crates/telima-docker/src/engine.rs`, `crates/telima-ui/src/engine_views/mod.rs`

### 컨테이너 로그

- "로그 보기"로 연다. stdout과 stderr를 함께, 시각(timestamps)을 붙여 받는다.
- "시각" 스위치: 각 줄 앞에 로컬 시각을 보인다.
- "따라가기" 스위치: 켜면 새 줄을 계속 받고 맨 아래로 이동한다("따라가는 중 · 새 줄이 오면 맨 아래로 이동"). 끄면 "따라가기 꺼짐 · 지금까지의 로그".
- "로그 검색": 입력한 글자가 들어간 줄만 보인다. 대소문자는 구분하지 않는다. 아래에 "6 / 17줄"처럼 보인 줄 수와 전체 줄 수를 보인다.
- `level=error`, `[error]` 같은 오류 수준 표시가 있는 줄은 오류 색으로 보인다.
- 메모리에는 최근 5,000줄만 둔다.
  근거: `crates/telima-ui/src/engine_views/mod.rs`의 `LOG_KEEP`, `crates/telima-app/src/view/daily.rs`의 `log_matches`
- Esc 또는 닫기 버튼으로 닫는다.

## 이미지

- "이미지" 탭. 열: "태그", "ID", "크기", "컨테이너". 크기가 큰 순서로 보인다. 태그가 없으면 `<none>`이다.
- "미사용 이미지 정리": 태그 없는(dangling) 이미지를 지운다. 확인 창에 개수와 크기를 보인다. 대상이 없으면 "정리할 미사용 이미지가 없습니다."
- 개별 이미지 삭제는 없다.
  근거: `crates/telima-docker/src/engine.rs`의 `prune_images`(`dangling=true`)

## 볼륨

- "볼륨" 탭. 열: "이름", "크기", "사용". 사용은 "사용 중", "미사용", "확인 불가"다. 이름순으로 보인다.
- "미사용 볼륨 정리": 어떤 컨테이너도 쓰지 않는 볼륨을 이름 있는 볼륨까지 지운다(`all=true`). 대상이 없으면 "정리할 미사용 볼륨이 없습니다."
- 개별 볼륨 삭제는 화면에 없다.
  근거: `crates/telima-docker/src/engine.rs`의 `prune_volume_filters`

## 캐시 정리

- 왼쪽 아래 "캐시 정리". 확인 창: "Colima가 내려받아 둔 VM 이미지 등 캐시 파일을 지웁니다. 프로필, 컨테이너, 볼륨에는 영향이 없고 필요하면 다음 시작 때 다시 내려받습니다."
- 실행 명령: `colima prune -f`

## Colima 버전 확인

- 앱을 시작할 때 `colima version`으로 설치된 버전을 읽어 기준과 비교한다. 최소 지원 버전 0.10.3, 확인된 최신 버전 0.10.3.
  근거: `crates/telima-core/src/version.rs`의 `MIN_SUPPORTED`, `LAST_TESTED`
- 최소 버전보다 낮으면: 오류 배너 "Colima 업데이트 필요"와 `brew upgrade colima`. 닫을 수 없다.
- 확인된 최신보다 높으면: 경고 배너 "검증되지 않은 Colima 버전". "닫기"로 닫으면 그 실행 동안 다시 나오지 않는다.
- Colima가 없으면: "Colima가 설치되어 있지 않습니다."와 `brew install colima`.
- `-rc1` 같은 접미사는 무시하고 해당 릴리스로 본다.
- 저장소의 `.github/workflows/colima-release.yml`이 새 Colima 릴리스를 감시한다. 앱 기능은 아니다.

## 트레이와 툴바

### 트레이 메뉴

위에서부터:

1. 오류 행(마지막 작업이 실패했을 때): "오류: <프로필> 시작 실패 — 창에서 보기" 또는 "오류: 프로필 목록을 읽지 못함 — 창에서 보기". 누르면 창이 열린다.
2. 요약: "Colima 실행 중 N · 중지 M". 누를 수 없다.
3. 프로필마다 "<이름> — <상태>" 하위 메뉴와 "시작", "중지", "재시작".
4. "창 열기", "새로고침", "telima 종료".

- 아이콘은 한 번에 하나: 오류, 작업 중, 실행 중, 모두 중지 순으로 우선한다. 툴팁은 "telima", "telima, 작업 중", "telima, 오류".
- 트레이에서 시작·중지·재시작·새 프로필을 누르면 창을 열고 창 안에서 실행한다. 진행 로그를 보이기 위해서다.
- 트레이의 "시작 중" 같은 표시는 트레이에서 보낸 명령일 때만 정확하다. 창에서 시작한 작업은 "처리 중"으로 보인다.
  근거: `crates/telima-app/src/view/tray.rs`, [tray.md](tray.md)

### 툴바

- 프로필 드롭다운: 툴팁 "프로필 선택: <이름>, <상태>". 마지막 항목 "새 프로필…".
- 선택한 프로필에 맞는 "시작", "재시작", "중지" 버튼, 그리고 "새로고침", "새 프로필". 작업 중에는 진행 상태와 경과 시간을 보인다.
- 툴바 빈 곳을 끌면 창이 옮겨지고, 두 번 누르면 macOS는 제목 막대 두 번 누르기 설정을 따르고 Linux는 창을 최대화·복원한다.

## 창 닫기와 종료

- 트레이가 있으면 창을 닫아도 앱은 계속 실행된다. 실행할 때마다 처음 닫을 때 한 번 알림을 보낸다: "telima는 메뉴 막대에서 계속 실행됩니다. 종료하려면 메뉴의 'telima 종료'를 누르세요." 알림은 macOS `osascript`, Linux `notify-send`로 보내며 없으면 생략한다.
- 트레이를 띄울 수 없는 Linux 환경에서는 창을 닫으면 앱이 끝난다.
- macOS에서 Dock 아이콘을 누르면 닫힌 창이 다시 열린다.
- 종료: 트레이 "telima 종료", macOS 앱 메뉴 "telima"의 같은 항목, 또는 ⌘Q(Linux는 Ctrl+Q). 단축키는 창에 초점이 있을 때만 동작한다.
  근거: `crates/telima/src/quit.rs`, `crates/telima/src/main.rs`

## 화면 모드

- 창은 시스템의 밝게·어둡게 설정을 따른다. 앱 안에서 고르는 설정은 없다.
- `TELIMA_APPEARANCE=light|dark`는 디버그 빌드에서만 동작하는 개발용 변수다. 릴리스 빌드는 무시한다([dev.md](dev.md)).

## 단축키

| 키 | 동작 |
|---|---|
| ⌘Q (Linux: Ctrl+Q) | 종료 (창에 초점이 있을 때) |
| Esc | 확인 창, 프로필 드롭다운, 로그 패널 닫기 |
| Enter | 프로필 폼 제출, 삭제 확인(이름을 맞게 입력했을 때) |
| ↑ / ↓ / Enter | 프로필 드롭다운에서 항목 이동과 선택 |

## 오류 안내

작업이 실패하면 창 위쪽에 오류 배너를 보인다. 주요 문구:

- "Colima VM 또는 Docker 엔진에 연결할 수 없습니다. 프로필이 실행 중인지 확인해 주세요."
- "요청한 항목을 찾을 수 없습니다. 목록을 새로고침한 뒤 다시 시도해 주세요."
- "작업을 완료하지 못했습니다. 잠시 후 다시 시도해 주세요."
- "다른 작업이 진행 중입니다. 끝난 뒤 다시 시도해 주세요."

근거: `crates/telima-app/src/view/notice.rs`

## 알려진 한계

- 단일 실행을 강제하지 않는다. 두 번 실행하면 창과 트레이 아이콘이 둘 생긴다.
- macOS 메뉴 막대가 꽉 차면 트레이 아이콘이 "«" 뒤로 접힌다. 앱은 이를 알아낼 수 없다.
- Linux 트레이는 StatusNotifierItem 호스트가 있어야 한다. GNOME은 AppIndicator 확장이 필요하다. Linux 밝은 패널용 아이콘은 확인하지 못했다.
- 창 닫기 안내는 설정 저장소가 없어 실행할 때마다 다시 나온다.
- 런타임은 docker만 지원한다.
- 편집 폼에서 마운트 방식, Rosetta, IP 주소도 바꿀 수 있지만 저장은 `colima.yaml`의 `cpu`, `memory`, `disk`, `mounts`, `kubernetes`만 고친다.
- 개별 이미지·볼륨 삭제는 화면에 없다. 정리(prune)만 있다.
- `TELIMA_APPEARANCE`, `TELIMA_COLIMA_VERSION`은 디버그 빌드 전용이다.
- 앱은 서명·공증하지 않는다.
