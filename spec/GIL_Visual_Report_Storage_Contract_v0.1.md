# T1.2 — 시각 자료 저장·공유 참조 계약안

상태: 계약안과 단계별 구현 기록. 현재 개발 체크아웃은 명시적 opt-in의 Project format 5
저장/close 기초(§12), 정적 PNG 디코딩·객체 보존 계층(§13), Unix 파일 경계와
실제 Session Verify 첨부·commit(§14), 후속 조회·경계 보호(§15–24)를 구현했다.
기본 생성·기존 Project는 format 4를 유지한다.
**2026-10-14 제품 목표 확정(미구현):** 지원되는 향후 작성 후보의 신규 생성은 format 5,
기존 4는 백업 준비/원본 적용을 따로 승인받아 전환한다. 현재 기본값과 구분하며
[일반 사용자 진입 계약](GIL_Visual_Report_Migration_v0.1.md) §5를 정본으로 따른다.
**N1/N2 개발 구현:** 명시적 library `ProjectSession::start_visual`은 최초 state부터 5를
배타적으로 저장한다. 기존 `.gil`의 재사용·자동 이관은 하지 않는다. §6의 합성 검수 범위이며
CLI/MCP의 기본 생성과 설치본은 여전히 4다. 도구 연결은 다음 단계다.
**후속 N3:** 개발 feature의 MCP `gil_start`는 위 생성 Core에 연결했다(이관 계약 §7).
기본 빌드·CLI·기존 설치본은 계속 4이며 실제 사용자 기록의 자동 이관은 없다.
공개 CLI/MCP 이미지 첨부·이미지 응답·Monitor 표시·설치본에는 아직 적용하지 않았다.
실제 사용자 데이터 이관은 별도 승인 대상이다.
§8 이후의 미완 표기는 각 체크포인트 당시 상태이며 최신 구현 장부는 §24를 따른다.
다음 표시 경로는 [보고서 이미지 표시 계약안](GIL_Visual_Report_Display_Contract_v0.1.md)을 따른다.
기준: [시각 보고서 제안](GIL_Visual_Report_Proposal_v0.1.md), Storage/Artifact/Cycle Model.

## 1. 세 개의 서로 다른 사실

1. **세계의 파일:** Verify가 이미지 또는 생성 코드를 만들거나 수정한다. 기존 Artifact 추적
   범위와 Snapshot 규칙을 따른다. 이미지 첨부가 추적 범위를 자동으로 바꾸지 않는다.
2. **확정 자료:** Verify close가 실제 이미지 bytes와 출처를 불변 자료로 등록한다.
   생성 코드만 있고 이미지가 없으면 이미지 첨부를 완료한 것으로 보지 않는다.
3. **보고서 참조:** 열린 Report가 이미 확정된 자료를 인용한다. 파일을 다시 읽거나 만들지 않는다.
   같은 자료라도 설명과 사용 목적은 Report마다 다를 수 있다.

세계에 있던 이미지가 나중에 삭제·교체되거나 restore되어도 확정 자료는 그대로 남는다.
자료 보존은 해당 파일을 세계에 다시 복구한다는 뜻이 아니다. Artifact 범위에서 제외된 파일을
첨부할 경우에도 명시적 경로·권한 검증은 필요하며, 그 첨부가 세계 Snapshot에 포함됐다고
보고해서는 안 된다. 추적 범위 선택과 첨부 선택은 별개다.

## 2. 논리 자료형 (이름은 아직 공개 API가 아님)

| 개념 | 필수 사실 | 불변식 |
| --- | --- | --- |
| VisualAsset | Project 내 ID, 생성 Verify, bytes digest, MIME, 길이, 크기, 출처 종류 | 생성 Verify close와 함께 확정, 이후 수정 불가 |
| VisualUse | asset ID, caption, alt, 용도, 표시 순서 | 이 Report의 close 시 고정, 자료 출처를 덮어쓰지 않음 |
| VisualDeclaration | 상태, 사유, VisualUse 목록 | 새 Verify에서 반드시 존재 |

자료는 Project가 소유한다. 생성 출처는 하나의 Verify이고 소비자는 여러 Step/Cycle Report다.
ID는 digest나 경로가 아니며, ID를 안다는 사실 자체는 조회 권한이 아니다.
같은 bytes의 물리 저장은 중복 제거할 수 있지만, 서로 다른 Verify의 생성 출처를 합치지 않는다.
원본 출처에 대한 주장(`captured`, `generated`, `provided`)과 서버의 bytes 검증을 구분한다.
기존 자료 재사용은 새로운 관측이 아니다. 변경·재캡처는 새 자료 ID와 출처를 만든다.

### Verify 선언

| 상태 | 요구 사항 | 거절 조건 |
| --- | --- | --- |
| attached | 하나 이상의 유효한 VisualUse | 빈 목록, 존재하지 않는 자료 |
| not_needed | 시각 자료가 불필요한 이유, 빈 목록 | 공백 사유, 첨부와 모순 |
| missing | 필요한 자료의 누락/생성 실패 이유와 후속 설명 | 불필요로 위장, 공백 사유 |

일부만 첨부하고 일부를 놓쳤다면 missing에서 기존 첨부 목록과 누락 설명을 함께 보존한다.
선언 자체가 없는 새 Verify는 close를 거절한다. 다른 종류의 Report는 선언을 강제하지 않지만
기존 자료를 참조할 수 있다. 구형 기록의 미기재는 `legacy_unspecified`라는 읽기 상태로
구분하며, 과거 Report에 "없음"이나 임의 사유를 쓰지 않는다.

missing은 자동 실패 판정이 아니다. 실제 성공 조건 충족 여부는 기존 Analysis/Outcome에서
판단한다. 필수 성공 조건을 충족하지 못한 실험을 성공으로 만들지도 않는다.

## 3. 생성과 공유의 권한

- 새 파일에서 bytes를 등록하는 동작은 열린 Verify의 close 경로에서만 허용한다.
- 다른 Step과 최종 Cycle Report에서는 기존 자료 참조만 허용한다. Report 본문의 Markdown,
  file URL, digest 문자열을 이미지 읽기 권한으로 해석하지 않는다.
- 참조 대상은 같은 Project에서 이미 확정된 자료여야 한다. 현재 Verify가 새로 등록하는
  자료에 대한 자기 Report 참조는 동일 트랜잭션 안에서 허용한다.
- 이전 실패 갈래의 자료도 Journey에 남는다. 명시적 참조는 가능하지만 현재 세계의 화면으로
  둔갑시키지 않는다. 생성 Verify를 표시한다. 시간 순서를 Cycle ID 비교만으로 추정하지 않는다.
- 닫힌 Report로의 연결 추가·교체·삭제 API는 제공하지 않는다.
- App은 명시적으로 연 scope, Report 소유 노드(Step 또는 Cycle), 해당 Report에 연결된
  자료 ID를 함께 제시한다. 서버는 연결과 scope를 매번 검증한다.
- 한 자료가 여러 Report에 쓰여도 다른 Project나 미참조 Report를 통한 조회는 거절한다.

## 4. close 원자성

프로젝트의 기존 쓰기 잠금 안에서 다음 순서를 따른다.

1. 열린 Verify/Step/Cycle과 기존 dirty gate·Report 규칙을 검사한다.
2. 새 자료 경로는 Verify에서만 읽는다. 경계·파일 종류·크기·동일 파일 안정성·bounded decode를
   검사한다. 읽은 bytes로 digest와 디코딩을 계산한다. 경로 재조회로 다른 bytes를 저장하지 않는다.
3. Verify의 세계 Snapshot과 자료 확정본이 모순되지 않도록 관측 안정성을 확인한다.
   추적 파일이 검사 사이 바뀌면 거절하며 같은 시점의 원자 Snapshot이라고 거짓 주장하지 않는다.
4. 검증된 객체를 내구성 있게 저장한다. Report 검증이 실패해도 논리 상태를 부분 확정하지 않는다.
5. 자료 registry·Report 참조·필수 선언·기존 Graph/Journey/Snapshot 변경을 하나의 state
   commit으로 확정한다. state를 먼저 쓰고 객체를 나중에 저장하지 않는다.
6. commit 이후에만 완료 receipt를 반환한다.

객체 저장 후 state commit 전에 중단되면 미참조 객체만 남을 수 있다. 초기 구현은 확정 자료를
GC하지 않는다. 손상·디스크 부족을 만났을 때 자료 없이 성공 처리하는 fallback은 금지한다.
commit 뒤 응답이 유실된 경우 새 ID로 close를 재실행하지 말고 상태를 읽어 완료 여부를 확인한다.
기존 잠금은 외부 편집기를 막지 않으므로 경합 시험을 별도로 수행한다.

## 5. 저장 호환 제안 — 아직 적용하지 않음

현재 Report는 문자열 맵이며 StoredNode/StoredCycle은 unknown field를 거절한다.
확정 자료 registry와 typed 참조는 단순 표시 옵션이 아니라 영구 의미이므로 **새 format을
명시적으로 도입하는 방안**을 권고한다. 문자열 필드에 JSON을 숨겨 우회하지 않는다.

- 신형 reader는 format 4를 읽되 과거 선언은 미기재로 보존한다.
- format 4의 읽기·Monitor 열기는 이관을 일으키지 않는다.
- 자료 기능을 사용하는 쓰기 전에 명시적 이관 승인을 받는다. 자동 다운그레이드는 없다.
- 이관은 기존 Graph·Report 원문·Journey·Snapshot·pending 의미를 보존하고 registry를 빈
  상태로 추가한다. 과거 Verify에 자료 또는 "불필요" 판정을 발명하지 않는다.
- 이관 전 전체 필요한 기록/객체의 복구 가능한 백업을 검증한다. state 한 파일만 복사하고
  완전한 백업이라고 부르지 않는다. 활성 restore 작업 등 중간 상태에서는 이관하지 않는다.
- 구형 writer가 신형 format을 쓰지 못함을 실제 이전 바이너리로 검증한다.
- 백업 복원은 이관 후의 새로운 기록을 잃을 수 있으므로 자동 복원하지 않는다.

정확한 format 번호, CLI close 인자, MCP schema/action surface와 이관 도구는 구현 전에
변경 범위를 함께 승인받는다. 기존 시험판·사용자 프로젝트는 이 계약안으로 변경되지 않는다.

## 6. Monitor와 후속 시각화

### 2026-10-13 기능 연결과 공통 보안 작업의 구분

이미지 기능의 직접 경로는 Verify의 첨부 검증/확정, 불변 자료 보존, Report별 참조 권한,
읽기 전용 Monitor 조회, 제한된 이미지 응답과 Node Detail 표시다. 현재 Session 첨부와
내부 Monitor 이미지 조회는 구현됐지만 공개 MCP 이미지 응답과 UI 연결은 아직 미완이다.
ReadOnlySession 기반 이미지 조회는 recovery/rollback/cleanup을 수행하지 않는다.

§19–24의 공통 Session/복원 보호는 사용자 파일 손실 위험을 줄이는 별도 기반 개선이다.
이 개선을 실제 이미지 표시가 진행됐다는 근거로 삼지 않는다. 남은 transaction 준비/전환
보호를 무한히 확장하지 않고, 다음 개발 게이트는 독립 합성 format-5 Project의 실제 이미지
표시로 한다. 기존 사용자 데이터의 자동 이관이나 미검수 Windows 첨부 허용은 하지 않는다.
MCP 응답 크기·scope/Report 권한·HTML 비실행·손상 거절은 화면 연결 단계에서도 필수다.

전체 Graph에는 큰 bytes를 넣지 않는다. 선택한 Report의 참조 목록을 먼저 읽고 자료는
한도 내에서 필요할 때 요청한다. Step 상세뿐 아니라 **최종 Cycle Report 상세**에도 같은
표시 계약이 필요하다. 현재 Step 전용 NodeDetail을 확장하지 않은 채 완료로 세지 않는다.
텍스트만 보기, 명시적 없음, 구형 미기재, 누락, 자료 손상/로딩 실패를 서로 구별한다.

성공한 C1에서 추가 자료 요청이 생기면 C1 종료 → C2 Define에서 목적과 기존 자료 참조 →
C2 Verify에서 자료 생성 → C2 최종 Report 순서로 간다. C1은 수정하지 않고 세계를 되돌리지
않는다. C1이 열려 있으면 먼저 정상 종료한다. 실제 실패는 기존 실패 경로를 따른다.

## 7. 구현 조각과 시험 게이트 (실행 범위는 §8)

1. 순수 선언/참조 검증: 세 상태·공백 사유·부분 누락·구형 미기재·존재하지 않는 참조.
2. 저장 이관: 구형 fixture 원문 보존, 신형 round-trip, 구형 writer 거절, 중단/백업 복구.
3. Verify close: Snapshot 일치·파일 경합·실패 원자성·재시도·응답 유실·디스크 부족.
4. 공유: 하나의 자료를 Verify/Outcome/Cycle/다음 Define에서 참조, 서로 다른 caption,
   원본 교체·삭제·restore 뒤 동일 bytes, 생성 출처 유지, 닫힌 Report 연결 수정 거절.
5. 보안 읽기: 미등록 scope·다른 Project·미참조 노드·임의 ID·path traversal·Windows reparse.
6. UI: Step/Cycle 상세, stale 응답, 선언 상태 구분, fullscreen 여백, 재시작·실제 Host 표시.

순수 검증 → 저장/이관 → Core → MCP → UI 순서로 진행한다. 문서 검사나 모형 시험을 제품
통합 시험, 이전 바이너리 호환 시험 또는 실제 화면 검수로 대체하지 않는다.

## 8. 첫 구현 조각 — 순수 검증 모듈

`src/visual_report.rs`에 `VisualDeclaration`, `VisualUse`, `VisualAssetId`와 검증 함수를
추가했다. 새 Verify 선언 누락, 빈 첨부, 공백 사유/후속 설명, 부분 누락, 참조 불가 ID,
빈 설명, 같은 Report의 중복 ID를 검사한다. 같은 자료를 서로 다른 Report가 별도의 목적과
설명으로 참조하는 것은 허용한다. 참조 목록 순서와 텍스트 원문을 변경하지 않는다.

이 조각의 구체적인 규칙: caption·alt·purpose는 비어 있지 않아야 하며, 한 Report 안에서
같은 자료 ID는 한 번만 나열한다. 같은 자료를 다른 Report가 참조하는 데는 이 제한이 없다.
단위 시험 12개가 통과했다. 입력 변형 없이 검증한다는 사실도 검사한다.

**연결되지 않은 것:** 기존 Report/close/store, 자료 registry, 파일 읽기/디코딩, 실제 Project
권한 검증, 이관, MCP, UI. resolver는 이후 통합 계층이 현재 Project의 확정 자료만 제공해야
하는 경계다. 가짜 resolver를 이용한 단위 시험은 보안 scope 검증의 통과 증거가 아니다.
serde/문자열 parser를 두지 않았으므로 새 저장 형식·wire 문법은 아직 확정하지 않았다.
§7 중 순수 검증의 해당 항목만 실행됐고 나머지는 미실행이다.

회귀 실행(2026-10-07): `cargo test --locked -p gil`은 29개 결과 묶음 총 1,188 통과,
0 실패. `cargo build --locked -p gil --all-targets` 통과, 새 파일 rustfmt 검사와
`git diff --check` 정상. 첫 sandbox 내 lib 실행은 370 통과/50 실패였으며, 독립 재현에서
loopback bind가 `Operation not permitted`로 거절됐다. 권한을 허용한 전체 재실행에서 통과했다.
JS·패키징·Host 화면·Tauri 검수는 이번 조각에서 실행하지 않았다.

## 9. 두 번째 코드 조각 — 메모리 registry

`src/visual_report/registry.rs`는 생성 Verify의 메타데이터와 Step/Cycle별 Report 참조를
보존한다. `prepare_close`는 원본을 변경하지 않고 검증된 후보 registry를 반환한다.
자료 생성·이름 발급·Report 연결이 하나라도 실패하면 후보를 버릴 수 있다. 닫힌 Report에
연결을 추가하는 메서드는 없으며, 이미 등록된 Report에 대해 다시 close 후보를 만들 수도 없다.

동일 bytes라도 다른 Verify에서 생성했다면 논리 ID와 출처를 분리한다. 같은 자료의 재사용은
새 생성이 아니라 명시적 참조다. 자료 ID만으로 조회하는 대신 Report와 실제 연결됐는지 검사한다.
자료 불필요 선언도 Report를 확정한 사실로 남으므로 나중에 첨부로 바꾸지 못한다.

추가 단위 시험 16개: 후보 폐기·오류 후 ID 보존, 부분 등록 없음, Verify 외 생성 거절,
경계 노드/종류 불일치/닫힌 구형 노드 거절, 공유 참조와 생성 출처 보존, 미참조 조회 거절,
ID 소진, 미참조 신규 자료 거절, 부분 누락, 선언 종류 구분. 이전 12개와 합쳐 28개 통과.

**남은 연결:** 이 registry는 아직 Project 상태에 포함되지 않는다. Graph 상태 인자는 이후
통합 계층이 실제 Project에서 읽어야 하며 Host 입력을 신뢰해서는 안 된다. metadata의 digest,
bytes 길이, MIME·크기는 decoder/object 계층의 검증이 아직 필요하다. 메모리 후보의 불변성을
디스크 원자성으로 부르지 않는다. 기존 format 4와 설치본에는 영향이 없다.

다음 조각은 registry의 엄격한 저장 DTO와 복원 검증이다. 별도 sidecar를 독립적으로 저장해서
Report close와 부분 확정될 수 있는 우회 경로는 만들지 않는다. 이후 새 state format과의
단일 commit 및 명시적 이관을 연결해야 실제 기능으로 사용할 수 있다.

회귀 실행(2026-10-07): 전체 `cargo test --locked -p gil` 29개 결과 묶음 1,204 통과,
0 실패. 기존 loopback 시험을 위해 권한을 허용해 실행했다. all-targets build, 새 Rust 파일
형식 검사, `git diff --check` 통과. 저장 이관·실제 bytes·Host 화면 검수는 아직 미실행이다.

## 10. 세 번째 코드 조각 — DTO codec와 복원 검증

`src/visual_report/registry/codec.rs`는 domain 타입과 분리된 엄격한 JSON DTO의 encode/decode를
제공한다. DTO version 1은 실험용 registry 문서의 판이며 **Project format 번호가 아니다.**
현재 `state.yaml`이나 별도 sidecar 파일을 읽고 쓰지 않는다. 최종 state envelope/이관 구현에
편입하기 전 단계이며 외부 도구 문법으로 공개하지 않는다.

- 순서대로 발급된 자료 ID·next ID, 중복 Report, 참조 문법과 실재, 생성 Verify의 역참조를 검사한다.
- 알 수 없는 필드/상태/형식, 중복 JSON 필드, 잘린 입력, 후행 쓰레기, 0 크기 메타데이터를 거절한다.
- 공유 참조와 출처·목록 순서·원문을 보존하고 encode/decode 후 결정적 출력인지 검사한다.
- metadata 문서는 잠정 16 MiB로 제한한다. decode 전에 크기를 확인하며 encoder의 출력도
  한도를 넘기기 전에 중단한다. 이미지 bytes/픽셀 안전성의 한도나 검증을 대체하지 않는다.
- 복원에는 `VisualGraph` 증거가 필요하다: 실제로 닫힌 Report 종류, close 당시 참조 가능 여부,
  선언이 필수인 새 Verify 목록. 새 Verify 선언을 삭제한 것을 구형 미기재로 허용하지 않는다.

codec 시험 23개, 시각 자료 모듈 합계 51개 통과. 시험의 Graph 증거는 합성 구현이다.
**실제 Project Graph adapter와 이관 경계는 아직 미연결**이며, registry 문서가 스스로 구형
Report 여부를 주장하게 해서는 안 된다. 빈 registry를 구형 프로젝트에 적용하는 것과 새 Verify
선언이 빠진 손상 상태를 구별할 책임은 새 state format과 migration이 함께 가진다.

메타데이터 digest를 파싱했다고 이미지 bytes의 진위·무결성이 검증된 것은 아니다. object 읽기,
bounded decoder, 저장 후 재읽기, 디스크 원자성·crash 복구 시험은 이후 조각에서 수행한다.
이 단계에서도 기존 format 4, 도구 schema, 사용자 프로젝트, 설치본은 변경하지 않았다.

회귀 실행(2026-10-07): `cargo test --locked -p gil` 29개 결과 묶음 총 1,227 통과,
0 실패. loopback 시험을 위해 권한을 허용해 실행했다. all-targets build, Rust 형식 검사,
`git diff --check` 통과. JS·패키징·Host 화면·Tauri 시험은 이번 변경에서 실행하지 않았다.

## 11. 네 번째 코드 조각 — 실제 Project Graph 읽기 연결

`src/visual_report/project.rs`의 `ProjectVisualGraph`가 codec의 합성 Graph 증거를 실제
Project 조회로 대체한다. Report 존재·종류·닫힘과 생성 시점은 실제 Cycle/Step/Journey에서
읽는다. 사용자가 보낸 Node kind나 ID 번호 대소 관계를 믿지 않는다. 열린 Verify를 위한
registry 후보는 만들 수 있지만 Graph에서 실제로 닫히기 전에는 codec 저장 검증을 통과하지 못한다.

동일 Existence의 Journey revision으로 생성/소비 순서를 확인한다. 생성 Verify 자체의 참조는
허용하고 다른 Report는 그보다 나중에 닫혀야 한다. 서로 다른 Existence의 revision은 비교하지
않고 거절한다. 향후 Existence 전환 시의 공유 계약을 이 구현으로 완료됐다고 주장하지 않는다.

`MigrationBoundary::capture`는 Existence별 현재 Journey 판을 **메모리 이관 계획**으로
잡는다. 그때까지 닫힌 Verify는 구형 미기재, 이후 닫힌 Verify는 선언 필수다. 이관 시점에 열려
있던 Verify도 나중에 닫히면 선언이 필요하다. 프로젝트보다 미래의 경계는 거절한다.
이 경계는 접근 권한이나 Project 식별자가 아니다. 파일 이관에 연결할 때 같은 잠금·동일 Project
안에서 한 번 확정하고 보존해야 하며, 매 읽기마다 재캡처해서 새 선언 의무를 지워서는 안 된다.

실제 Project 통합 시험 7개 통과: 상태/종류 조회, 열린 Verify의 이관 경계, 미래 경계 거절,
Journey 시점 검사, 실제 Verify close→다음 Define 공유 참조→codec 왕복, 구형/신형 Verify
구분, format 4 파일 읽기·계획 후 원본 바이트 불변. 합성 프로젝트만 사용했다.

**아직 아닌 것:** Project 구조의 registry 소유 필드, 실제 close의 일괄 반영, state DTO 변경,
백업/이관 명령, 이미지 객체 저장/디코딩. 따라서 디스크 write 경로는 여전히 format 4 그대로다.
이 다음에는 이 경계와 registry를 새 state 형식에 함께 넣고, 부분 저장이 불가능한 close/commit
경로와 명시적 백업·이관을 연결해야 한다. 실제 사용자 프로젝트의 자동 변환은 하지 않는다.

회귀 실행(2026-10-07): Core 전체 30개 결과 묶음 1,234 통과, 0 실패. all-targets build,
변경 파일 형식 검사, `git diff --check` 통과. 기존 loopback 시험에 필요한 권한을 허용했다.
JS·패키징·Host 화면·Tauri는 이번 코드 조각에서 검수하지 않았다.

## 12. 다섯 번째 코드 조각 — Project 소유와 단일 state 저장

Project가 선택적인 `VisualState`(이관 경계와 registry)를 소유한다. 읽기 조회만 제공하며
닫힌 Report의 시각 참조를 직접 수정하는 API는 없다. `prepare_visual_upgrade`는 원본을
그대로 둔 메모리 후보를 반환하고 이미 전환된 Project의 경계를 다시 캡처하지 않는다.
이는 **최종 사용자 이관 명령이 아니다**. 잠금·백업·실패 복구를 갖춘 이관은 아직 미구현이다.

명시적 후보의 저장 형식은 5다. `state.yaml`의 같은 envelope에 `visuals.boundary`와
`visuals.registry`를 함께 직렬화한다. registry 내부 DTO version 1과 Project format 5는
서로 다른 판이다. 기존 `save`의 임시 파일·fsync·rename 경로를 사용하며 독립 sidecar는 없다.
시각 상태 없는 Project의 생성/저장은 여전히 format 4이며 읽기/쓰기가 자동 전환하지 않는다.
format 5의 시각 상태 누락, format 4에 추가한 시각 registry, 선언 누락, 미래/중복/알 수 없는
Existence의 이관 경계, 알 수 없는 registry version은 복원 시 거절한다. 읽으며 수정하지 않는다.

`close_step_with_visuals`는 Report·Will·Journey·Snapshot과 시각 선언을 복제 후보 위에서
검증한 뒤 한 번에 반영한다. 새 Verify의 기존 close 호출은 선언 없음을 거절한다. 새 Report가
잘못되거나 선언이 잘못되면 전체 Project가 이전 그대로다. `close_cycle_with_visuals`도 같은
원칙을 사용한다. 성공한 Cycle 뒤 새 Cycle을 열 수 있지만 열린 Cycle을 병렬로 열 수는 없다.

저장 통합 시험 8개는 합성 Project만 사용한다: 원본 불변·이관 재실행 거절, format 4 바이트
동치, format 5 왕복과 경계 유지, close 실패 원자성·닫힌 Report 재변경 거절, not_needed와
missing 구별, 구형 Project의 시각 선언 묵살 금지, 손상 상태 거절, Cycle 종료/후속 전환.

**아직 아닌 것:** 이미지 bytes 객체 저장·디코딩·무결성 검증, 자료 생성 등록을 받는 Project
API, Session의 첨부 close, CLI/MCP 문법, Monitor resource/rendering, 사용자 백업 이관 명령.
현재 Project close 경로는 검증되지 않은 이미지 metadata를 신규 등록받지 않는다. codec의
metadata 복원은 이미지 파일이 존재하거나 안전함을 증명하지 않는다. JSON codec의 16 MiB
입력 사전 제한과 달리 YAML envelope는 기존 state 파서 후 registry 직렬화 크기를 검사한다.
대용량 입력 사전 방어·crash/fault injection·실제 이미지 보존 검수는 이후 연결에서 수행한다.
이미지 표시 기능이 완료되거나 배포 가능하다는 뜻이 아니며 사용자 기록/설치본은 건드리지 않았다.

회귀 실행(2026-10-07): `cargo test --locked -p gil` 31개 결과 묶음 1,242 통과,
0 실패. all-targets check/build, 새 Rust 파일 형식 검사, `git diff --check` 통과.
첫 컴파일에서 DTO Debug derive와 새 StoreError의 source match 누락을 보정했다.
JS·패키징·Host 화면·Windows·Tauri 검수는 이번 단계에서 실행하지 않았다.

## 13. 여섯 번째 코드 조각 — PNG bytes 검증과 객체 보존

`visual_report/image.rs`의 `VerifiedImage`는 정적 PNG 전체 디코딩을 통과한 원본 바이트를
소유한다. 생성자는 digest·길이·크기를 사용자 metadata에서 받지 않고 동일 바이트에서 계산한다.
경로·URL은 입력받지 않으며 `Read`의 경로 허가와 파일 종류 검증은 호출 계층의 책임이다.
디코딩 성공은 실제 관측 여부나 자료 공개 동의를 입증하지 않는다. 출처 종류/설명은 작성자의
주장으로 보존한다. Debug 출력에는 이미지 원문 바이트를 넣지 않는다.

초기 한도는 encoded 16 MiB, 가로/세로 각각 8,192, 전체 16,777,216 픽셀, decoded buffer
128 MiB, decoder 내부 best-effort 32 MiB, PNG chunk 4,096개다. 입력은 최대 한도+1 byte로
제한하며 초과하면 거절한다. 메모리 한도가 프로세스 전체의 강제 격리 또는 실행 시간 제한을
뜻하지는 않는다. CRC와 Adler-32 검증을 켜고 ancillary CRC 실패도 묵인하지 않는다.
최종 IEND 뒤 쓰레기·잘림·손상·APNG·다른 포맷을 거절한다. JPEG는 논리 enum에 남아 있지만
이 ingest 경로는 아직 받지 않는다. SVG/HTML/script는 실행하거나 이미지로 변환하지 않는다.

검증은 원본을 재인코딩하지 않는다. 원래 PNG의 metadata도 바이트 그대로 보존하므로 민감정보
제거·외부 공유 승인이 이 검증으로 대체되지 않는다. 향후 화면 제공 시 raster MIME/CSP와
Project/Report 범위를 확인하는 별도 resource 경계를 유지해야 한다.

내부 `ObjectStore::put_visual/read_visual`은 기존 content-addressed blob 저장소를 재사용한다.
쓰기 뒤 되읽어 주소/바이트를 확인하고, 읽기에서는 출력 크기 제한·digest·디코딩·metadata
일치를 재검사한다. 기존 동일 주소 객체는 덮어쓰지 않으며 손상을 자동 복구하지 않는다.
새 디스크 namespace나 GC는 도입하지 않았다. 미참조 객체는 논리 Report 확정이 아니다.

**중요한 연결 경계:** 두 저장 메서드는 crate 내부 기초이며 Session의 잠금·storage-tree guard
아래에서만 호출하도록 설계했다. 아직 제품 호출 경로에는 연결하지 않았다. 기존 ObjectStore의
경로 안전성을 이 메서드만으로 강화했다거나 임의 파일 경로를 안전하게 첨부할 수 있다고
주장하지 않는다. 다음 단계는 명시적 프로젝트 상대 경로 검증, Unix no-follow/Windows reparse,
동일 handle 관측 안정성, Snapshot과의 일치, Verify close 및 state commit 연결이다.
MCP/UI/사용자 이관 기능은 계속 미노출이다.

전용 시험 13개: 실제 디코딩과 byte 보존, 모든 prefix 잘림·fixture byte별 손상, ancillary CRC,
후행 데이터, 미지원 형식/APNG, 무한 reader의 용량 제한, 차원/픽셀 수/chunk 수 각각의 제한,
읽기 실패, 원본 교체·삭제 후 창고 재열기, 중복 저장, 누락·손상·metadata 불일치·자동 수리 금지,
객체 출력 상한. 합성 임시 파일만 사용했으며 실제 사용자 이미지를 복사하지 않았다.

디코더는 workspace lock에 이미 있던 `png 0.18.1`을 Core의 직접 의존성으로 고정했다.
Core 배포 graph에는 png와 전이 의존성 8개 package가 추가됐다(두 miniz_oxide 판 포함).
macOS·Windows x64/ARM64 고지 정책과 원문을 함께 갱신했고 기존 원문은 삭제하지 않았다.
설치된 Plugin에 Node/Cargo/외부 이미지 프로그램을 추가 요구하지 않는다.

검증(2026-10-07): 전체 Core 31개 결과 묶음 1,255 통과/0 실패, all-targets build 통과.
`distribution/codex/*.test.mjs`·공용 고지·`distribution/windows/*.test.mjs` 합계 84 통과/0 실패.
새 Rust 파일 formatting 및 whitespace 검사 통과. 처음에는 시험에서 private hex helper를
호출한 컴파일 오류와 IEND를 중복 삽입한 차원 시험 fixture 실패가 있었고, 시험 코드를 고친 뒤
집중/전체 시험을 재실행했다. Windows native 실행·MCP/Host 이미지 표시·디스크 부족/중단
fault injection·실제 사용자 파일 첨부는 검수하지 않았다. 커밋·push·설치·배포는 하지 않았다.

## 14. 일곱 번째 코드 조각 — 파일 경계와 Verify 첨부 transaction

`ProjectSession::close_verify_with_images_and_commit`은 format 5의 실험용 라이브러리
진입점이다. 기존 close와 달리 디스크 commit까지 끝나야 receipt를 반환한다. CLI/MCP
도구는 아직 이 메서드를 부르지 않는다. 실제 사용자 프로젝트 이관이나 설치본 변경도 없다.

- 열린 Verify, 명시적 format 전환, 최대 8개 입력을 먼저 확인한다.
- 경로는 명시적인 Project 상대 경로만 받는다. 절대 경로·상위 이동·역슬래시·제어 문자·
  Windows drive/ADS 구문·예약 장치 이름과 `.gil`/`.git` 내부 접근을 거절한다.
- Unix에서는 canonical root의 절대 경로부터 `openat`/`O_NOFOLLOW`로 각 구성 요소를 연다.
  부모는 directory여야 하며 최종 파일은 일반 파일이고 hard-link 수가 1이어야 한다.
  `O_NONBLOCK`으로 FIFO를 열다 무한히 기다리지 않으며 디렉터리·특수 파일은 읽지 않는다.
- 동일 handle의 dev/inode/link 수/길이/mtime/ctime와 현재 이름의 identity를 비교한다.
  Snapshot 전후 및 객체 저장 후 바이트를 다시 읽어 검사한다. 같은 바이트의 다른 파일로
  바뀌어도 거절한다. 첨부 전체 encoded 용량은 32 MiB로 제한한다.
- Snapshot이 추적하는 파일은 첨부 digest와 일치해야 한다. 추적 범위에 없는 파일의 첨부가
  추적 범위를 확장하거나 Snapshot에 포함됐다는 주장이 되지 않는다.
- registry·Report·Will·Journey·Snapshot을 복제 후보에서 검증하고, 객체 보존·재검증을
  먼저 수행한 뒤 같은 state envelope를 저장한다. 성공한 state 저장 뒤 메모리 상태를 갱신한다.

검사 실패는 기존 논리 Report를 변경하지 않는다. 객체 저장 이후 state 저장 이전에 실패하면
미참조 객체가 남을 수 있다. rename 이후 sync/응답 단계의 실패는 commit 결과가 불명확할 수
있으므로 반드시 상태를 다시 읽어 판정해야 한다. 오류 뒤 close를 무조건 반복하지 않는다.
외부 편집기는 GIL 잠금에 참여하지 않으므로 파일시스템 전체의 한순간 원자 Snapshot 보장은
아니다. 최종 검사 뒤 생긴 외부 변경까지 막는다고 주장하지 않는다.

**보안 경계 정정과 남은 게이트:** §13에서 기대한 storage-tree guard는 현재 Session/저장소
전체에 descriptor 기반으로 구현된 보장이 아니다. 이번 no-follow는 새 첨부 source 경로를
보호하며 기존 Artifact capture/ObjectStore/state 쓰기 전체를 hardening한 것이 아니다.
실험 API는 신뢰된 내부 `.gil` tree를 전제로 한다. 내부 저장 경로의 악의적 동시 교체,
저장된 자료를 읽어 외부로 제공하는 scope/resource 권한, 사용자 백업 이관은 배포 전 별도
게이트로 남긴다. 이를 닫기 전에 새 이미지 MCP 도구를 노출하거나 안전 검수 완료로 세지 않는다.

Windows에서는 동등한 handle/reparse 경계가 검수되기 전까지 새 파일 첨부를 거절한다.
기존 Windows 기능과 무첨부 선언의 순수 도메인 규칙은 바꾸지 않는다. macOS 시험으로 Windows
파일 첨부 지원을 주장하지 않는다.

합성 Session 시험 10개: 실제 첨부/출처/state 재로딩·원본 삭제 후 byte 보존, 닫힌 Verify 재호출,
잘못된 Report/선언, 준비 중 파일 수정, 동일 내용 파일 교체, leaf/parent symlink와 hardlink,
unsafe path·directory/FIFO·중복 경로, state 쓰기 실패, 명시적 not_needed, Verify/format 사전
거절, 개수 제한. 공용 경로 단위 시험 1개도 추가했다. callback으로 경합을 결정적으로 주입했으며
모든 OS race를 망라하는 스트레스 시험은 아니다.

검증(2026-10-07): Core 32개 결과 묶음 1,266 통과/0 실패, all-targets build 통과.
Windows x64 `cargo check --locked --offline -p gil --lib --target x86_64-pc-windows-msvc`
통과(실행 시험 아님). 배포/고지 회귀 84개 통과/0 실패, whitespace 검사 통과.
이번 집중/전체 시험의 실패는 없었다. Windows native 실행·Host 표시·실제 사용자 파일은
검수하지 않았고 커밋·push·설치·배포도 하지 않았다.

## 15. Report 범위의 보존 이미지 조회 — 실험 API

`ProjectSession::read_report_image(owner, asset_id)`를 추가했다. 사용자 파일 경로나 digest를
입력받지 않는다. 현재 Session의 registry에서 해당 Report가 실제로 참조한 asset만 찾아
저장 객체 이름을 계산한다. 원본 파일이 삭제돼도 보존본을 반환하며 state는 변경하지 않는다.

Unix에서는 `.gil`부터 blob까지 모든 내부 구성 요소를 no-follow로 열고, 최종 객체의 일반
파일 여부·hard-link 수·identity/변경 시각을 검사한다. 바이트는 제한된 PNG 전체 디코딩을
거쳐 digest·길이·형식·크기가 registry와 일치해야 하며 반환 전 같은 handle을 다시 읽는다.
정상 PNG로 바꿔치기해도 digest가 다르면 거절한다. Windows 조회는 동등한 경계 검수 전까지
거절하며, x64 cross-check는 컴파일 검수일 뿐 native 실행 증거가 아니다.

이 API는 **Host의 Project/scope 권한을 대신하지 않는다**. 호출자는 별도로 scope를 검증해야
한다. Session을 열 때의 기존 경로 처리나 ObjectStore/state 쓰기를 안전하게 바꿨다는 뜻도
아니다. §14의 내부 저장소 쓰기·권한·이관 게이트는 그대로 남아 있고 MCP 도구는 노출하지 않았다.

회귀 3개를 추가했다: 원본 삭제 후 조회·다른 Report/알 수 없는 asset/다른 Project 거절 및
state byte 불변, `.gil`부터 leaf까지 각 단계 symlink와 hardlink 거절, 손상/다른 정상 PNG/
누락/디렉터리 거절. 합성 Session 시험은 총 13개 통과했다. 첫 컴파일에서 시험 코드가
`VisualAssetId`의 NonZero 입력 타입을 잘못 사용해 실패했고 수정 후 통과했다.

검증(2026-10-07): Core 전체 32개 결과 묶음 1,269 통과/0 실패, all-targets build 통과,
Windows x64 library cross-check 통과, whitespace 검사 통과. 새 의존성은 없으며 배포물·
설치본·실제 사용자 프로젝트는 바꾸지 않았다. GUI 표시·Windows native 실행은 미검증이다.

## 16. Monitor의 명시적 Project scope와 조회 연결

`ReadOnlySession`에도 동일한 Report 이미지 조회를 연결했다. Monitor 내부 후보
`report_image(scope, owner, asset)`는 기존 저장된 선택 동의와 Project identity 검사로
Session을 연다. 이미지 전체 검증 뒤 잠금을 유지한 채 scope identity를 다시 검사한 다음
바이트를 반환한다. 알려지지 않은 scope나 바뀐 Project는 이미지 조회 전에 거절한다.
연결되지 않은 asset과 읽기 실패는 경로 없는 `visual_unavailable`로 반환한다.

이 후보는 **tools/list·resources/read에 등록하지 않았다**. 기존 Monitor binding의 권한
모델을 재사용하는 것이며 별도 사용자 인증·악의적 동시 파일 교체 전체 방어를 추가했다는
뜻은 아니다. Session opening과 내부 ObjectStore/state 쓰기의 경계 보강은 여전히 미완이다.
읽기 전용 경로는 recovery·commit을 수행하지 않는다. 외부 MCP 요청의 크기/호출 빈도 제한,
UI 이미지 수명·캐시와 실제 화면 검수도 이후 연결 단계의 과제다.

합성 시험에서 unknown scope, Report 연결 없음, 같은 위치의 `.gil` 교체를 거절했고
경로를 오류로 내보내지 않음을 확인했다. 실제 보존 이미지는 ReadOnlySession으로 읽었고
state bytes가 바뀌지 않았다. 외부 사용자 파일·설정·설치본은 사용하지 않았다.

검증(2026-10-08): Core 전체 1,271 통과/0 실패, all-targets build 및 Windows x64
library cross-check 통과, whitespace 검사 통과. Windows native·Host 화면은 미검증이며
이번 변경을 커밋·push·설치·배포하지 않았다.

## 17. Unix format-5 state envelope 쓰기 경계

새 시각 Report 형식의 Unix `save`는 기존 `.gil` 디렉터리를 각 구성 요소 no-follow로
연 뒤 그 descriptor를 유지한다. Project root의 OS 별칭은 먼저 canonicalize하되 `.gil`
자체는 canonicalize하지 않는다. 저층 format-5 저장 대상은 기존 Project의 `.gil` 바로
아래 파일이어야 한다. 일반 임의 위치의 파일 내보내기 API로 사용하지 않는다.

임시 이름은 `O_CREAT|O_EXCL|O_NOFOLLOW`, 권한 0600으로 생성한다. 그 이름이 이미 있으면
일반 파일·symlink·hardlink·디렉터리 어느 것이든 거절하고 기존 내용을 보존한다.
보유한 handle에 쓰고 sync한 뒤 바이트를 재검증하고, 임시 이름의 inode/link 수를 검사한다.
`renameat`의 양쪽 이름은 같은 디렉터리 descriptor에 상대적이다. 목적지 symlink는
엔트리로 교체할 뿐 대상에 쓰지 않는다. 부모 이름의 identity를 교체 전후 다시 검사한다.

실패한 임시 파일은 자동 삭제·덮어쓰기하지 않는다. 다음 시도는 이를 거절하므로 사용자
이관/복구 안내가 필요하다. rename 이후 sync 또는 identity 검사 실패는 이미 저장됐을
가능성이 있어 상태 재판독이 필요하다. 같은 사용자 권한의 악의적 프로세스가 보유 파일의
내용을 동시에 바꾸는 모든 경합을 원자적으로 봉쇄한다는 보장은 아니다.

**범위:** format-4와 Windows 저장 동작은 바꾸지 않았다. Windows의 새 첨부는 계속 거절한다.
Artifact capture/blob/manifest의 기존 쓰기, Session opening, 사용자 이관/복구는 아직
보호 완료가 아니다. 새 MCP 이미지 도구와 UI는 여전히 비노출이다.

추가 회귀 3개: 기존 임시 일반 파일/symlink/hardlink의 내용과 이전 state 보존, `.gil`
부모 symlink 거절 및 대상에 임시 파일을 만들지 않음, 최종 파일 symlink 교체 시 외부 대상
내용 보존과 저장 state 재로딩. 합성 첨부/조회/저장 통합 시험은 총 17개 통과했다.

검증(2026-10-08): Core 전체 1,274 통과/0 실패, all-targets build 통과, Windows x64
library cross-check 통과, whitespace 검사 통과. 이번 시험의 실패는 없었다.
Windows native·GUI·사용자 프로젝트 이관은 미검증이고 커밋·push·설치·배포하지 않았다.

## 18. Unix 객체 저장·재조회 경계

Unix ObjectStore의 blob/manifest 공통 publish를 descriptor 상대 연산으로 바꿨다.
이 경로는 format-4와 format-5 모두 사용하므로 기존 Snapshot·복원 시험도 회귀 범위다.
Windows 구현과 저장 형식·객체 주소·도구 schema는 바꾸지 않았다.

- 선택된 Project root 아래 `.gil/artifacts`부터 tmp·객체 종류·sha256·shard까지 no-follow로
  연다. 새 디렉터리는 `mkdirat` 후 부모를 sync한다. 기존 링크를 따라 생성하지 않는다.
- 임시 파일은 0600·`O_EXCL`로 만들며 보유 handle에 스트리밍/hash/sync한다.
- 확정은 `linkat`의 no-clobber 동작이다. 기존 객체를 교체하거나 rename으로 우회하지 않는다.
  같은 내용을 동시에 확정하는 기존 사용례도 유지한다.
- 임시 이름 identity, 객체 부모 및 저장소 identity를 검사하고 확정 바이트를 다시 hash한다.
  읽기도 descriptor-relative no-follow로 열고 일반 파일인지 검사한다. 읽기 오류 이전에
  외부 symlink 대상의 바이트를 호출자에게 흘려보내지 않는다.
- 실패 시 정리는 보유한 임시 파일과 현재 이름이 같은 객체일 때만 시도한다. 다른 이름으로
  교체된 항목은 보존한다. 임시 정리 자체가 원래 작업 성공을 뜻하지 않는다.

시험 4개 추가: 내부 디렉터리 링크(다섯 위치), 최종 객체 링크, 입력 스트리밍 중 저장소
디렉터리 교체, 입력 스트리밍 중 임시 이름을 외부 파일 링크로 교체. 링크 대상 내용 보존,
읽기 출력 없음, 교체된 임시 이름 보존을 확인했다. 기존 동시 확정·손상 객체 불변·저장 후
hash 재검증도 유지한다. 첫 집중 실행에서 권한 거절 뒤 임시 파일이 남는 회귀가 발견되어
identity를 검사하는 정리 guard를 추가했고 재시험은 통과했다.

**남은 한계:** 같은 사용자 권한의 악의적 동시 writer에 대한 완전한 race-free 트랜잭션은
아니다. 외부 작업 트리 관측, Session lock/open, recovery/reap, format-4 state 쓰기는 이번
보강 범위 밖이다. 일부 검증/응답 실패는 미참조 객체를 남길 수 있다. Windows 동등 구현,
사용자 백업·이관·실패 복구 UX, MCP 자료 전달/제한과 화면 검수는 여전히 미완이다.

검증(2026-10-08): 최종 Core 전체 1,278 통과/0 실패, all-targets build 및 Windows x64
library cross-check 통과. UI/Plugin JS 90건 중 86 통과·4 기존 skip, 배포/고지/Windows
후보 회귀 84 통과/0 실패, whitespace 검사 통과. Windows native·실제 Host 이미지 표시·
실제 사용자 이관은 검수하지 않았다. 커밋·push·설치·배포하지 않았다.

## 19. Unix Session 입구의 제어 파일 검사

Unix에서 `.gil` bootstrap은 Project root를 기준으로 `mkdirat`하고 no-follow directory
handle을 연다. 기존 `.gil` 링크는 따라가지 않는다. 잠금 파일은 같은 디렉터리 기준으로
열며, 일반 파일·hard-link 수 1을 확인한다. advisory lock을 얻은 뒤 보유한 디렉터리와
파일 identity가 현재 이름과 같은지 다시 확인한다.

Session open/read-only open은 상태 파일을 미리 열어 symlink·hardlink·특수 파일을 거절한다.
이 검사는 recovery와 tmp 정리보다 먼저다. 상태 파일이 없는 경우는 기존 복구/load 절차에
맡긴다. Unix state load도 no-follow handle로 읽고 읽기 전후 이름의 identity를 검사한다.
저층 load는 `.gil` 바로 아래 파일을 대상으로 한다. 저장소 밖 임의 YAML 읽기 경로가 아니다.

`load_within`/`save_within`은 더 이상 잠금을 타입 표식으로만 받지 않고 읽기/쓰기 전후에
실제 잠금 identity를 검증한다. 잠금 파일이 교체됐다면 과거 handle을 가지고 commit하지
않는다. 저장 후 identity 확인 실패는 이미 저장됐을 수 있으므로 상태 재판독이 필요하다.

합성 회귀 4개: 잠금 symlink/hardlink 거절, 상태 symlink/hardlink/directory/FIFO 거절,
`.gil` 링크의 start/open 거절, 잠근 뒤 lock 교체 시 commit 거절. 오류 전에 tmp 정리를
수행하지 않았고 외부 대상과 이전 state가 그대로인지 확인했다. FIFO는 nonblocking으로
열고 파일 종류 검사에서 거절한다.

범위는 Unix이며 Windows는 기존 구현을 유지한다. 기존 format-4도 Session 입구 및 읽기
검사를 사용하지만 state 쓰기는 아직 이전 방식이다. recovery/reap 자체의 모든 경로 연산을
descriptor 방식으로 바꾼 것은 아니다. 같은 권한의 외부 writer를 완전히 차단하거나
파일시스템 전체의 원자적 Snapshot을 보장하지 않는다. 사용자 이관/복구 UX와 MCP/UI
노출 검수는 여전히 남아 있다.

검증(2026-10-08): Core 전체 33개 결과 묶음 1,282 통과/0 실패, all-targets build 및
Windows x64 library cross-check 통과. 배포/고지/Windows 후보 회귀 84 통과/0 실패,
whitespace 검사 통과. 첫 빌드의 Unix 미사용 import 경고는 cfg 분리로 제거했다.
Windows native·실제 사용자 이관·Host 이미지 표시는 미검증이며 설치·배포하지 않았다.

## 20. Unix 임시 객체 정리와 복구 영역 입구

macOS/Linux의 내부 디렉터리 열거는 `fdopendir/readdir`로 보유 descriptor 자체를 읽는다.
`readdir`의 오류와 정상 끝을 구분하고 스트림 handle을 닫는다. 다른 Unix 플랫폼은 이 열거
방식이 검수되기 전에는 거절한다. Windows는 기존 구현을 유지한다.

Unix tmp 정리는 `.gil/artifacts/tmp`를 각 단계 no-follow로 열고 다음 순서로 수행한다.

1. 보유 디렉터리에서 이름을 모은다.
2. 모든 이름이 기존 `<pid>-<ticket>` 규칙이고 일반 파일인지 검사하고 handle을 유지한다.
3. 현재 이름이 유지한 inode와 같은지 다시 검사하고 `unlinkat`으로 제거한다.
4. 디렉터리를 sync한다.

unknown 이름이나 링크·특수 항목이 최초 검사에 있으면 하나도 지우지 않고 거절한다.
유효한 tmp 파일의 hardlink는 unlink만 수행하며 대상 내용을 덮어쓰지 않는다. 보유 handle
기준이므로 부모 경로가 링크로 바뀌어도 그 링크를 따라 삭제하지 않는다. 검사와 unlink
사이에 같은 권한의 외부 writer가 이름을 바꾸는 모든 경합을 원자적으로 막는 보장은 아니다.

restore 영역의 발견 단계도 같은 descriptor 열거를 사용하며 영역 루트와 transaction
루트가 링크이면 기존 rollback/cleanup에 들어가기 전에 거절한다. **실제 rollback의 모든
파일 이동 및 nested backup/plan 접근이 descriptor 기반으로 전환된 것은 아니다.** 이
차이를 닫지 않고 복구 전체 보안 검수 완료로 보고하지 않는다.

추가 합성 회귀 4개: artifacts/tmp 부모 링크 대상의 파일 보존, unknown/linked 항목이 있는
경우 전체 항목 보존, 정상 tmp만 제거하며 state byte 불변, restore 영역/active 링크 대상
보존. Session 경계 집중 시험 총 8개 통과. 실제 사용자 프로젝트나 설치본은 사용하지 않았다.

검증(2026-10-08): Core 전체 1,286 통과/0 실패, all-targets build 및 Windows x64
library cross-check 통과, 배포/고지/Windows 후보 회귀 84 통과/0 실패, whitespace 검사
통과. 이번 시험의 실패는 없었다. Windows native·GUI·실제 사용자 이관은 미검증이며
커밋·push·설치·배포하지 않았다.

## 21. Rollback 원본 무결성과 Unix 복구 제어 파일

rollback은 첫 작업 파일을 변경하기 전에 **모든** 백업을 복구 계획의 `before` digest와
대조한다. 손상된 뒤쪽 백업을 뒤늦게 발견해 앞쪽 파일부터 바꾸는 일을 피한다. 파일 수만큼
descriptor를 유지하지 않고 하나씩 검사한 뒤, 실제 복사 시 다시 열고 복사 결과의 digest를
다시 검사한다. 데이터는 파일 크기와 무관하게 고정 크기 버퍼로 hash한다.

Unix에서는 `.gil/restore/<transaction>/PLAN`, `COMMITTED`, `backup/<id>`를 각 단계
no-follow로 열며 제어 파일/백업 hardlink도 거절한다. 완료 표식의 링크를 완료로 간주하거나
계획 파일의 링크를 해석하지 않는다. 보유한 원본 handle에서 복사하고, 옆자리 임시 파일은
`create_new`로 만들어 기존 항목을 덮어쓰지 않는다. 복사본 hash가 맞아야 최종 rename한다.

공통 rollback의 백업 hash/복사본 검사는 Windows에도 적용되지만 경로 no-follow 보장은
Unix만 구현했다. Windows native 검수는 별도다. 동일 권한의 동시 writer가 사전 검사 후
뒤쪽 백업을 바꿀 경우 일부 rollback 뒤 거절할 수 있다. 이때 transaction을 보존하며 성공으로
보고하지 않는다. 계획 전체의 진위 인증을 새로 도입한 것은 아니며, 목적지 부모/권한 변경/
rename과 cleanup의 모든 경합을 막은 것도 아니다.

추가 복원 회귀 3개: 손상된 백업을 전체 사전 검사에서 거절, backup 파일 symlink/hardlink
및 backup 디렉터리 링크 거절, PLAN/COMMITTED 링크 거절. 실제 crash failpoint로 active
transaction을 만든 뒤 조작했으며 거절 후 작업 트리와 state, 보관 자료가 남는지 확인했다.
복원 집중 시험은 29개 통과했다. UI 이미지 노출·사용자 이관·설치·배포는 하지 않았다.

검증(2026-10-08): 최종 Core 전체 1,289 통과/0 실패, all-targets build와 Windows x64
library cross-check 통과, 배포/고지/Windows 후보 회귀 84 통과/0 실패, whitespace 검사
통과. Windows native·Host 이미지 표시는 미검증이다. 커밋·push는 하지 않았다.

## 22. Unix rollback 목적지 경계

rollback의 대상 경로 전체를 쓰기 전에 검사하고, Project root와 부모 디렉터리를
no-follow descriptor로 연다. 대상 파일/부모가 링크 또는 특수 항목이면 거절하며,
없는 부모는 원본 복원 시에만 descriptor 기준으로 만든다. 새 파일 제거는 `unlinkat`,
원본 복원은 같은 부모의 배타적 임시 파일에 복사·digest 검사 후 `renameat`로 수행한다.
권한 변경도 임시 파일의 보유 handle에만 적용한다. 이번에 만든 빈 디렉터리만 제거한다.

루트/부모의 이름과 보유 handle의 동일성을 작업 전후 검사한다. 같은 권한의 외부 writer가
검사와 unlink/rename 사이에 이름을 바꾸는 모든 경쟁을 원자적으로 방어한다는 보장은 아니다.
실패 시 transaction과 복구 증거를 보존하며, 중간에 생성한 임시 파일이 남을 수 있다.
정상 forward apply 및 transaction cleanup 전체를 전환한 것은 아니다. Windows 목적지
경로 보호, 사용자 이관, MCP/UI 이미지 노출 역시 별도 미완 항목이다.

추가 합성 회귀 2개는 실제 crash 뒤 대상 파일/부모 링크를 심어 첫 쓰기 전에 거절하는지,
rollback의 새 파일 삭제가 부모 링크 너머 외부 파일을 삭제하지 않는지 검사한다.
외부 sentinel, state bytes, active PLAN 보존을 확인했다. 복원 집중 시험 31개 통과.

검증(2026-10-08): Core 전체 1,291 통과/0 실패, all-targets build 및 Windows x64
library cross-check 통과. JS 86 통과/기존 skip 4, 배포·고지·Windows 후보 회귀 84 통과,
whitespace 검사 통과. Windows native·Host 화면은 검수하지 않았다. 커밋·push·설치·배포 없음.

## 23. Unix transaction 잔여물 전체 사전 검사

cleanup은 `.gil/restore/<transaction>`과 `backup`을 no-follow 디렉터리 handle로 열고,
PLAN·COMMITTED·숫자 이름의 일반 backup 파일만 허용한다. 알 수 없는 이름이나 링크,
특수 파일이 최초 검사에 있으면 어떤 파일도 삭제하기 전에 거절한다. 기존의 항목별
검사 즉시 삭제 방식과 달리 뒤쪽에서 unknown을 발견해 앞쪽 복구 자료를 잃지 않는다.

파일 수만큼 descriptor를 유지하지 않고 최초 dev/inode를 기록한 뒤 각 unlink 전에 다시
확인한다. 삭제는 보유 디렉터리 기준 unlinkat, 폴더는 AT_REMOVEDIR로 비었을 때만 한다.
검사 뒤 새 unknown이 추가되면 폴더 제거는 실패하며 남은 항목을 재귀 삭제하지 않는다.
같은 권한 writer와의 검사-삭제 사이 경쟁을 완전히 원자적으로 봉쇄한다는 보장은 아니다.
중간 I/O 실패 시 일부 정리가 끝났을 수 있으며 성공으로 보고하지 않는다.

합성 crash로 cleanup 잔여물을 만든 뒤 unknown 최상위 항목, backup 폴더 symlink,
backup 안의 unknown 항목을 각각 주입했다. 모두 기존 PLAN·COMMITTED·backup 바이트,
프로젝트 state와 세계, 외부 sentinel이 보존됐다. 복원 집중 시험 32개 통과.

이 변경은 Unix 잔여물 삭제 경로만 대상으로 한다. 정상 forward apply, transaction
준비/완료 표식 쓰기와 cleanup 이름으로의 전환, Windows 경계는 여전히 별도 검수 대상이다.

검증(2026-10-13): Core 전체 1,292 통과/0 실패, all-targets build 및 Windows x64 library
cross-check 통과, 배포 관련 84 통과/0 실패. 전체 시험 후 링크 시험의 백업 보관 위치를
transaction 밖으로 옮겨 unknown 이름에 의한 우연한 거절을 배제했고, 해당 시험을 재실행해
통과했다. 구현 코드는 전체 시험 이후 변경하지 않았다. whitespace 정상. GUI·Windows native
검수는 하지 않았고 커밋·push·설치·배포도 하지 않았다.

## 24. Unix 정상 복원 적용

적용 직전 digest 판독도 부모를 no-follow로 열어 수행한다. 정상 apply의 생성/교체는
rollback과 같은 descriptor 기반 게시 함수를 사용하며, 배타적 옆자리 파일에 blob을 복사하고
복사본 digest를 다시 검사한다. Replace 권한은 보유 file handle로 적용하고, Create는
기존대로 0666에서 시스템 umask가 적용된 권한을 쓴다. 삭제와 빈 부모 제거도 unlinkat
기반이다. target blob과 작업 파일의 inode를 공유하지 않는다.

새로 만든 부모만 made 목록에 기록하며 기존 폴더는 넣지 않는다. 안전한 접근은 canonical
root로 하되 목록의 경로 표기는 호출자의 root와 일치시켜 /var와 /private/var 같은 alias가
rollback의 이번 생성 폴더 판정을 깨뜨리지 않도록 한다. 이 차이는 새 시험의 최초 실패로
발견해 구현을 수정했다. 최초 컴파일의 Result error type 추론 문제도 명시 타입으로 수정했다.

복사 중 부모를 다른 자리로 옮기고 외부 폴더 symlink로 교체하는 결정적 시험에서 게시를
거절하고 외부 sentinel과 기존 원본을 보존했다. 같은 경로의 digest와 삭제도 거절한다.
새 중첩 폴더 기록과 정상 게시 시험, 기존 권한/중단/rollback 복원 집중 시험을 함께 검수한다.

이 변경은 prepare의 백업 생성, PLAN/COMMITTED 쓰기, transaction 이름 전환을 아직
바꾸지 않는다. 동일 권한 writer와 검사-rename/unlink 사이의 모든 경쟁을 원자적으로 막는
것도 아니다. 실패한 임시 파일이 남을 수 있으며 복구 필요 상태를 성공으로 보고하지 않는다.
Windows 실행 경로는 기존 구현을 유지하며 native 안전성 검수 완료로 세지 않는다.

검증(2026-10-13): Core 전체 1,294 통과/0 실패(33 test groups), all-targets build와
Windows x64 library cross-check 통과. 배포 관련 84 통과/0 실패, whitespace 정상.
최종 Core 실행에는 새 경계 시험 2개와 복원 집중 시험 32개가 포함된다. GUI·Windows native
실행은 미검증이며 커밋·push·설치·배포하지 않았다. 별도 영구 체크포인트를 보관한다.

## 25. Windows 핸들 기반 경계 — 단계 1 (2026-10-18)

Windows 지원 계약 `GIL_Windows_Visual_Support_Contract_v0.1.md`에 따라 §13–§20의 Unix 경계에
대응하는 Windows 구현을 추가했다. Unix 코드와 저장 형식·도구 schema·도구 이름은 바꾸지 않았다.

- 공용 원시 연산 `src/windows_fs.rs`: 각 구성 요소를 `FILE_FLAG_OPEN_REPARSE_POINT`로 항목
  자체로 열고 reparse 속성이면 거절, 조상 디렉터리 핸들 보유(이름 변경·삭제 공유 거부),
  `FileIdInfo`(volume + 128-bit id) identity, `FileBasicInfo` 시각 포함 `Stamp`, `CREATE_NEW`
  임시 파일, 보유 디렉터리 핸들 상대 `FileRenameInfoEx`(POSIX, 배타/교체), 핸들 삭제
  `FileDispositionInfoEx`, `CreateHardLinkW` no-clobber, 디렉터리 `FlushFileBuffers`,
  새 생성 전용 `DRIVE_FIXED` + NTFS 게이트.
- 첨부(`visual_report/file.rs`): Windows `open`은 root와 상대 경로의 모든 구성 요소를 pin한
  뒤 마지막 항목을 열고 일반 파일·링크 수 1을 요구한다. `same`은 `Stamp` 전체 일치다.
  공유 모드는 Unix처럼 제한하지 않고 변경은 `Stamp`·바이트 재검증으로 감지한다.
- state 저장(`state_file.rs`): Unix와 같은 checkpoint 이름으로 `.writing` 배타 생성 → flush →
  재판독 → 이름 재확인 → `.gil` 핸들 상대 rename(`write_new`는 교체 없음) → 디렉터리 flush.
  실패한 임시 파일은 보존한다.
- 객체 저장(`artifact/store_windows.rs`): `.gil/artifacts`를 pin한 뒤 tmp에 배타 생성, 목적지
  체인 pin, `CreateHardLinkW`로 확정, 임시 항목은 보유 핸들로만 삭제, 확정 객체 재판독.
  읽기도 같은 체인으로 열고 reparse·디렉터리를 거절한다.
- Session 입구(`lock.rs`, `session.rs` preflight, `store::load`): 잠금·상태 파일을 pin된 `.gil`
  아래에서 항목 자체로 열고 일반 파일·링크 수 1을 요구하며 identity를 전후 비교한다.
  `.gil` 핸들은 Unix descriptor처럼 삭제를 공유한다(이름 변경은 identity 비교로 감지).
- `visual_start.rs`: 예약을 플랫폼 모듈로 분리했다. Windows는 `CreateDirectoryW` 배타 예약,
  항목 자체 열기, root/`.gil` identity, 부모 flush. macOS 동작은 그대로다.
- 제외: 이관(`visual_migration`·`gil_migration_*`는 Windows에서 계속 거절), `reap_tmp`·복구·
  rollback의 Windows 핸들 경계, `.gil` 보호 DACL 적용 여부(계약 §8).

검증(2026-10-18): Mac 교차 `cargo check --target x86_64-pc-windows-msvc --all-targets`
default·visual-authoring 통과(기존 Unix 전용 시험 3곳에 `cfg(unix)` 게이트 추가). Windows
runner(PR #25, run 39126103310, head 09882352): x64·ARM64 × default·visual-authoring 4 job 통과 —
authoring job 은 binding 11, mcp 24, `visual_attachment` 22, `visual_start` 1, `visual_store` 8,
lib 필터 118(ignored 1 = abort 자식 진입점), 24-도구 smoke·format 5·재시작. 앞선 3회의 실패와
수정(rename 호출 형식, fixture, junction root 거절)은 Windows 지원 계약 §6.1 에 있다. macOS
run 39126103325 통과. runner 통과도 실제 PC·Host 화면 인수가 아니다.

보완(2026-10-18): `.gil` 은 생성 시 소유자 전용 보호 DACL(현재 사용자·SYSTEM, 상속 가능,
상위 상속 차단)로 태어나고 생성 직후·재열기마다 핸들로 실효 DACL 을 검증한다. 기존 Project 의
ACL 은 자동 변경하지 않으며 넓어진 format-5 저장소는 거절한다. 전체 경로 rename 의 보안 검토와
회귀시험, 다른 로컬 계정의 실제 접근 시험, UNC·매핑 드라이브 거절 시험을 추가했다(Windows
지원 계약 §6 W6/W7, §8, §9; runner 결과는 거기에 기록).
