# 개발용 오버라이드

리뷰·스크린샷용 환경 변수입니다. **디버그 빌드(`cargo run`, `cargo build`)에서만** 동작하며,
릴리스 빌드(`--release`)는 값을 읽어도 무시합니다. 시스템 설정(macOS 화면 모드 등)은 건드리지 않습니다.

| 변수 | 값 | 효과 |
| --- | --- | --- |
| `TELIMA_APPEARANCE` | `light` 또는 `dark` (대소문자 무관) | 창의 시스템 화면 모드 대신 해당 팔레트를 강제합니다. 다른 값이면 시스템 설정을 따릅니다. |
| `TELIMA_COLIMA_VERSION` | `X.Y.Z` (예: `0.9.1`, `v0.11.0`) | 시작 시 버전 확인이 이 버전을 받은 것처럼 동작합니다. 실제 Colima 호출(`colima version`)만 대체되고 나머지 명령은 그대로입니다. 형식이 틀리면 무시합니다. |

## 예시

```sh
# 어두운 팔레트
TELIMA_APPEARANCE=dark cargo run -p telima

# "업데이트 필요" 배너 (MIN_SUPPORTED 미만)
TELIMA_COLIMA_VERSION=0.9.0 cargo run -p telima

# "아직 검증되지 않음" 배너 (LAST_TESTED 초과)
TELIMA_COLIMA_VERSION=0.99.0 cargo run -p telima
```

두 값을 함께 지정해도 됩니다. 기준 버전은 `telima-core::version`의 `MIN_SUPPORTED`, `LAST_TESTED`입니다.

## 구현 메모

- 파싱은 순수 함수입니다: `telima_ui::style::{parse_appearance, resolve_appearance}`,
  `telima::dev_override::version_override`. 디버그 여부(`cfg!(debug_assertions)`)를 인자로 받아
  릴리스 동작도 단위 테스트로 확인합니다.
- 버전 오버라이드는 바이너리의 `VersionOverride`(`DynVm` 래퍼)가 `colima_version`만 바꿉니다.
