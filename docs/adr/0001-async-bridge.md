# ADR 0001: 뷰와 유스케이스를 잇는 비동기 다리

- 상태: 채택
- 날짜: 2026-11-11

## 배경

- 어댑터는 tokio가 필요하다. telima-docker는 bollard(=0.21.1)를 쓰고, telima-colima는 `tokio::process`로 CLI를 띄운다.
- GPUI의 실행기는 tokio가 아니다. tokio 런타임 밖에서 bollard 퓨처를 poll하면 실패한다.
- 계층 규칙(`scripts/check_layers.py`): telima-core·telima-app은 gpui/tokio/bollard를 모르고, telima-ui는 tokio/bollard/어댑터 크레이트를 모른다.
- 포트(`VmManager`, `ContainerEngine`)는 `impl Future + Send`를 돌려주므로 `dyn`으로 쓸 수 없다. 그대로 쓰면 유스케이스의 제네릭 인자(`ProfileService<V>`)가 뷰 타입까지 번진다.
- Docker 소켓은 프로필마다 다르다(`~/.colima/<profile>/docker.sock`). 엔진 하나를 앱 전체가 나눠 쓸 수 없다.

## 결정

telima-app에 `runtime` 모듈을 두고 다음을 정의한다.

1. `Spawner` (object-safe): `fn spawn(&self, work: BoxFuture<'static, ()>)`. 바이너리(telima)가 자기 tokio 런타임 핸들로 구현한다. 시험에서는 `futures::executor`로 구현한다.
2. `DynVm`, `DynEngine` (object-safe): 포트와 같은 메서드를 `BoxFuture`로 돌려준다. 모든 `VmManager`/`ContainerEngine`에 대한 blanket impl이 있어서 어댑터는 따로 할 일이 없다.
3. `App` 파사드 (`Clone + Send + Sync`): `Arc<dyn DynVm>`, 프로필별 엔진 팩토리(`Fn(&ProfileName) -> PortResult<Arc<dyn DynEngine>>`), `Arc<dyn Spawner>`를 받는다. 안에서 기존 `ProfileService`/`EngineService`를 그대로 쓴다(규칙·정렬·작업 직렬화의 정의는 여전히 한 곳).
4. `App`의 메서드는 값을 소유권으로 받고 바로 돌아온다. 유스케이스는 `Spawner` 위에서 돌고, 결과는 `done: FnOnce(Result<T, AppError>) + Send` 콜백으로, 진행 상황과 로그 줄은 `FnMut(..) + Send` 콜백으로 전달한다.
5. 모든 메서드는 `Task`를 돌려준다. `Task::cancel()`은 다음 await 지점에서 작업을 멈추고, 그 뒤로 `done`은 불리지 않는다. `Task`를 버려도 작업은 끝까지 간다.

콜백은 Spawner의 스레드에서 불린다. 뷰는 콜백 안에서 뷰 상태를 건드리지 않고, 채널로 값을 UI 스레드에 넘긴 뒤 GPUI 쪽 태스크에서 받아 `cx.notify()` 한다.

### 권장 형태에서 달라진 점과 이유

- 엔진 팩토리: 권장안은 `EngineService` 하나를 들고 있는 형태였다. 소켓이 프로필마다 달라서, 엔진 메서드는 `ProfileName`을 받고 호출 때마다 팩토리로 엔진을 얻는다. `EngineService`는 상태가 없어서 매번 만들어도 비용이 없다.
- `Task`(취소): 권장안에 없던 것이다. `logs(follow = true)`는 컨테이너가 멈출 때까지 끝나지 않으므로, 화면을 떠날 때 멈출 방법이 없으면 태스크와 연결이 샌다. `futures::future::abortable` 한 군데로 모든 메서드에 같은 방식으로 적용했다.

## 버린 선택지

- **telima-ui가 tokio 핸들을 직접 쓴다**: 계층 규칙 위반. 뷰 시험에도 tokio 런타임이 필요해진다.
- **뷰를 어댑터 타입으로 제네릭하게 만든다** (`MainView<V, E>`): 타입 인자가 모든 뷰와 엔터티로 번지고, GPUI 엔터티 타입이 어댑터 조합마다 달라진다.
- **포트를 `async-trait`(전부 boxed)로 바꾼다**: core의 포트 정의와 이미 있는 어댑터·가짜 구현을 다 고쳐야 한다. 정적 디스패치가 필요한 곳도 없어진다. 지금 방식은 포트를 그대로 두고 경계 한 곳에서만 box 한다.
- **퓨처를 돌려주고 GPUI 실행기에서 await 한다**: bollard 퓨처가 tokio 밖에서 poll되어 실패한다. JoinHandle을 await 하는 형태는 가능하지만 tokio 타입이 app/ui 시그니처에 드러난다.
- **async-compat 같은 호환 계층**: 숨은 전역 런타임이 생기고, 어느 스레드에서 무엇이 도는지 코드로 드러나지 않는다.
- **App이 채널(Stream)을 돌려준다**: 채널 구현 선택이 app의 공개 API에 굳는다. 콜백이면 UI가 자기 실행기에 맞는 채널을 고른다.

## 결과

- telima-ui는 `telima_app::runtime::App`만 안다. tokio·bollard·어댑터 의존이 없다. 뷰 시험은 `testing::FakeVm`/`FakeEngine`과 시험용 `Spawner`로 돈다.
- 바이너리가 할 일: tokio 런타임 생성, `Spawner` 구현(`handle.spawn(work)`), 어댑터를 `Arc`로 감싸 `App::new`에 전달.
- 호출마다 box 할당 1~2번과 가상 호출이 든다. CLI 실행·소켓 왕복에 비하면 무시할 수준이다.
- 포트에 메서드가 생기면 `DynVm`/`DynEngine`과 그 blanket impl, 안쪽 래퍼(`SharedVm`/`SharedEngine`)에도 한 줄씩 더해야 한다. 빠뜨리면 컴파일이 실패한다.
- 취소는 퓨처를 drop 하는 방식이다. 어댑터는 drop 될 때 자식 프로세스·스트림을 정리해야 한다(예: `kill_on_drop`). 프로필의 busy 표시는 guard가 drop 되면서 풀린다.
- `done` 콜백이 패닉하면 Spawner의 태스크가 죽는다. 콜백은 채널 전송만 하고 실패를 무시하는 것이 안전하다(받는 쪽 뷰가 이미 사라진 경우).
