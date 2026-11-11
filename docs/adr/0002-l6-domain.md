# ADR 0002: L6 일상 기능을 위한 도메인 타입과 포트

- 상태: 채택
- 날짜: 2026-11-11

## 배경

L6은 매일 쓰는 기능을 더한다: 프로필 가동 시간과 버전 표시, 시작할 때 Colima 버전 확인, 새 Colima 릴리스 알림, 컨테이너 시작/정지·포트·시작 시각, 로그 시각, 데이터 디스크까지 지우는 삭제, 디렉터리 마운트와 Kubernetes, `colima prune`.

확인한 사실(Colima 0.10.3):

- `colima version`은 colima 버전과 docker client/server 버전을 낸다. `colima status -j`에는 가동 시간이 없다.
- `colima delete --data`는 컨테이너 런타임 데이터까지 지운다. `colima prune`은 받아 둔 캐시를 지운다.
- Docker 목록 API는 포트와 생성 시각을 주고, 시작 시각은 inspect에서만 나온다.

telima-core는 gpui/tokio/bollard와 시간 라이브러리를 모른다(`scripts/check_layers.py`). 의존성은 `thiserror` 하나다.

## 결정

### 1. 시각은 `time::UnixMillis(i64)` 하나로 표현한다

- 유닉스 epoch 기준 밀리초. 어댑터가 RFC 3339·초 단위 값을 이 값으로 바꾸고, 뷰가 문자열로 만든다.
- 현재 시각은 항상 인자로 받는다(`elapsed_until(now)`). core는 시계를 읽지 않으므로 시험이 결정적이다.
- `now`가 더 이르면 0을 돌려준다. 호스트와 VM 시계가 어긋나도 음수 가동 시간이 나오지 않는다.

### 2. 실행 정보는 `RuntimeInfo`, 가동 시간은 시작 시각에서 계산한다

- 이름을 `ProfileStatus`로 하지 않았다. 이미 Running/Stopped를 뜻하는 enum이 그 이름을 쓴다.
- `started_at`을 저장하고 `uptime(now)`로 계산한다. 가동 시간을 값으로 들고 있으면 읽은 순간부터 틀려지고, 화면이 매초 포트를 다시 불러야 한다.
- 모든 필드는 `Option`이다. Colima가 가동 시간을 직접 주지 않으므로 어댑터가 못 구할 수 있고, 멈춘 프로필에는 server 버전이 없다. 모르는 값은 오류가 아니라 `None`이다.
- docker 버전은 문자열이다. 비교 규칙이 없고 표시만 한다.
- 포트는 둘로 나눴다: `colima_version()`(프로필이 하나도 없어도 시작할 때 확인 가능)과 `runtime_info(name)`(프로필별).

### 3. Colima 버전 범위는 `version` 모듈 한 곳에 둔다

- `ColimaVersion { major, minor, patch }`는 필드 순서대로 `Ord`를 파생해 숫자로 비교한다(0.9.9 < 0.10.0).
- `MIN_SUPPORTED`, `LAST_TESTED`는 둘 다 0.10.3으로 시작한다. `rules::MIN_COLIMA_VERSION`(튜플, 0.10.0)은 지웠다. 같은 사실의 정의가 둘이 되기 때문이다. 쓰는 곳은 없었다.
- `LAST_TESTED`는 한 줄짜리 `ColimaVersion::new(0, 10, 3)`로 둔다. 릴리스 감시 워크플로가 이 줄을 읽으므로 두 번째 사본이 필요 없다.
- `version_support(v)`는 순수 함수다: `MIN_SUPPORTED` 미만은 `TooOld`, `LAST_TESTED` 초과는 `Untested`, 나머지는 `Supported`.
- `parse`는 `v` 접두사를 받고 `-rc1`, `+abc` 같은 꼬리를 무시한다. 릴리스 후보는 그 릴리스로 본다. 숫자 세 개가 아니면 오류다(`HEAD-1234` 등). 오류를 어떻게 보여줄지는 app이 정한다.

### 4. 컨테이너: 포트·시각 필드, 상태별 가능 동작은 core가 정한다

- `Container`에 `ports: Vec<PortBinding>`, `created_at`, `started_at`을 더했다. 시각은 `Option`이다(구형 엔진·inspect 실패에서도 목록은 나온다).
- `PortBinding`의 호스트 쪽(`host_ip`, `host_port`)은 `Option`이다. expose만 된 포트는 호스트 포트가 없다. `is_published()`로 구분한다.
- `ContainerState::can_start()`(Created, Exited)와 `can_stop()`(Running, Paused, Restarting)을 core에 뒀다. 버튼 활성 조건을 뷰마다 다시 쓰지 않게 하려는 것이다. Dead는 시작할 수 없고 Paused는 unpause가 필요하므로 시작 대상이 아니다.
- `ContainerEngine`에 `start_container`, `stop_container`를 더했다.
- `LogLine.timestamp: Option<UnixMillis>`. 시각을 요청하지 않았거나 해석하지 못한 줄도 버리지 않는다.
- 로그 검색은 포트가 아니다. 이미 받은 줄을 거르는 일이라 app/뷰 모델의 몫이다.

### 5. 삭제 옵션은 `DeleteOptions { data }`

- `VmManager::delete(name, options)`. `bool` 인자 대신 구조체를 썼다. 호출부에서 `true`의 뜻이 보이고, 옵션이 늘어도 시그니처가 그대로다.
- 기본값은 `data: false`(디스크 보존)다. 시험으로 고정했다.

### 6. 마운트와 Kubernetes

- `ProfileSpec.mounts: Vec<Mount { path, writable }>`, `ProfileSpec.kubernetes: bool`. 빈 목록은 Colima 기본값(홈 디렉터리)이다.
- 규칙(`rules::validate_new`):
  - 경로는 `/`로 시작해야 한다(`MountPathNotAbsolute`). `~/src`도 거부한다. 펼치는 일은 입력을 받는 쪽에서 한다.
  - 같은 경로는 한 번만(`DuplicateMount`). 끝의 `/`와 `writable` 차이는 무시하고 같은 경로로 본다. 같은 중복은 한 번만 보고한다.
  - 디렉터리가 실제로 있는지는 어댑터가 본다. core는 모양만 본다.
  - Kubernetes는 docker 런타임에서만 허용한다(`KubernetesNeedsDocker`, 기준은 `KUBERNETES_RUNTIME`).
- 수정 규칙: 두 필드 모두 만든 뒤에 바꿀 수 있다. `IMMUTABLE_FIELDS`는 그대로다.
- 재시작이 필요한 필드는 `RESTART_FIELDS`에 적는다: cpus, memory_gib, disk_gib, mount_type, rosetta, network_address, mounts, kubernetes. Colima는 설정을 VM이 시작할 때 적용하므로, 바꿀 수 있는 필드는 전부 여기에 든다.
- `restart_required(current, next)`는 바뀐 필드 이름을 `RESTART_FIELDS` 순서로 돌려준다. 실행 중인 프로필이면 재시작 안내를 띄우고, 멈춘 프로필이면 다음 시작 때 적용된다. 실행 중인지는 app이 판단한다.

### 7. `VmManager::prune_cache()`

- 인자가 없다. 캐시는 프로필 공통이다. `--all`(Lima 자산 포함)은 사용자가 고르지 않았으므로 넣지 않았다.
- 결과는 `()`다. Colima가 지운 양을 구조화해 주지 않는다.

## 버린 선택지

- **core에 `chrono`/`time` 추가**: 필요한 것은 시각 한 점과 두 점의 차이뿐이다. 형식 변환은 어댑터와 뷰의 일이다.
- **`RuntimeInfo.uptime: Duration`**: 읽은 순간부터 낡는다. 위 2번 참고.
- **`Profile`에 버전·시작 시각 필드 추가**: `list`가 프로필마다 명령을 더 실행해야 하고, 그중 하나가 실패하면 목록 전체가 실패한다.
- **`semver` 크레이트**: 세 숫자 비교에 의존성 하나를 더하는 일이다. pre-release 순서 규칙은 여기서 쓰지 않는다.
- **새 포트 메서드에 기본 구현**: 어댑터와 가짜가 구현을 빠뜨려도 컴파일된다. ADR 0001과 같이, 빠뜨리면 컴파일이 실패하게 둔다.
- **`ProfileSpec`에 `Default`**: 새 필드를 호출부가 모른 채 지나간다. 이름·크기에 뜻 있는 기본값도 없다.
- **`delete_with_data` 메서드를 따로 추가**: 같은 동작의 포트가 둘이 된다.

## 결과

- 포트와 구조체가 바뀌어 telima-app, telima-colima, telima-docker, telima-ui가 고쳐질 때까지 컴파일되지 않는다. 고칠 곳은 핸드오프 문서에 있다.
- ADR 0001대로 `DynVm`/`DynEngine`, blanket impl, `SharedVm`/`SharedEngine`에 새 메서드를 한 줄씩 더해야 한다.
- 로그 시각을 받으려면 telima-docker가 로그 요청에 `timestamps`를 켜고 줄 앞의 RFC 3339 값을 떼어 `UnixMillis`로 바꿔야 한다.
- 가동 시간의 출처는 telima-colima가 정한다. 못 구하면 `started_at = None`이고 화면은 가동 시간을 숨긴다.
- `LAST_TESTED`를 올릴 때는 계약 시험을 그 버전에서 다시 돌린 뒤 이 상수 한 줄만 바꾼다.
- `ProfileSpec`에 필드를 더할 때는 `IMMUTABLE_FIELDS`나 `RESTART_FIELDS` 중 한 곳에 넣어야 한다. 컴파일러가 잡아 주지 않으므로 리뷰에서 확인한다.
