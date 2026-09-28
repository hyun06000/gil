# Contributing to GIL

GIL은 개발·검수 중이다. 정식 비개발자 설치는 marketplace 경로로 준비하고 있으며,
아래는 **기여자용** 절차다. 현재 검수 범위는 [README](README.md)와 [Roadmap](spec/GIL_Roadmap.md)을 본다.

## 변경 제안

- 문제, 재현 절차, 기대/실제 결과, OS/CPU와 사용한 Host의 모드를 함께 적는다.
- domain 계약은 `spec/`가 정본이다. 구현과 충돌하면 조용히 의미를 바꾸지 말고 먼저 드러낸다.
- 작은 변경 단위로 코드·회귀시험·필요한 문서를 함께 제안한다. 기존 사용자 변경을 섞지 않는다.
- 공개 issue/PR에는 token, 개인 경로, 사용자 대화, 실제 Project의 `.gil`, credential을 넣지 않는다.
  민감한 데이터 대신 최소한의 합성 fixture로 재현한다. 보안 취약점의 상세/악용 절차도 공개하지 않는다.

비공개 보안 제보 경로와 운영 정책은 아직 확정 중이며 정식 출시 전 게이트다.
현재 존재하지 않는 연락처나 응답 기한을 약속하지 않는다.

## 개발과 시험

현재 검증한 제작 환경은 macOS arm64, Rust 1.97.1, Node 22 이상이다. Node는 UI 제작과 비교시험용이며
native Plugin 설치본의 실행 의존성은 아니다. 새 환경의 필요 의존성·지원 범위는 별도 확인한다.

```sh
npm ci --prefix mcp-app
node mcp-app/build.mjs
cargo test --locked -p gil
cargo build --locked -p gil --all-targets
```

UI 빌드는 Plugin의 생성 HTML을 갱신한다. 의도하지 않은 차이는 포함하지 않는다. JS 회귀시험:

```sh
npm ci --prefix plugins/gil-companion-prototype
node --test ui/layout.test.mjs mcp-app/*.test.mjs plugins/gil-companion-prototype/*.test.mjs
node --test distribution/codex/artifact.test.mjs
```

실제 Project 조작 시험은 독립 임시 폴더에서 한다. GIL 자체 source 저장소에 `gil start`를 실행하거나
실제 사용자 기록을 시험 데이터로 쓰지 않는다. native MCP/화면 인수는
[NATIVE-ACCEPTANCE](mcp-app/NATIVE-ACCEPTANCE.md)와 [ACCEPTANCE](mcp-app/ACCEPTANCE.md)를 따른다.

Companion을 변경하면 해당 package의 시험과 실제 플랫폼 검수를 추가한다. `gil` 시험 성공을
Tauri 포함 workspace 전체 성공이라고 부르지 않는다. 시험마다 실행한 명령, 통과/실패/미실행,
첫 실패와 재실행을 구분한다. protocol 성공은 화면 표시의 증거가 아니다.

저장소 전체에는 기존 수동 formatting이 남아 있다. 기능 변경과 무관한 전체 `cargo fmt`를 섞지 않는다.
`git diff --check`로 whitespace를 확인하고 필요한 범위만 수정한다.

## 배포·공개 준비

현재 source는 MIT LICENSE이며 새 저장소는 비공개 이전 준비 중이다. 기여할 코드·fixture·이미지의 권한과 외부 의존성 고지를
확인한다. 별도 CLA나 라이선스 변경 정책을 이 문서로 새로 정하지 않는다.

서명되지 않은 개발 preview는 정식 출시물이 아니다. source 공개, GitHub CI artifact 공유,
marketplace 게시를 구분하며 [오픈소스 공개 준비 게이트](spec/GIL_Open_Source_Readiness_v0.1.md)를
정식 출시 전에 닫는다. 비밀·서명 키는 source, 공개 로그 또는 artifact에 포함하지 않는다.
