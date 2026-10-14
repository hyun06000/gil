# T1.2 — 노드 상세의 시각 자료 제안

상태: **불변 Report·사후 보충 금지 원칙 승인, 저장·도구 계약 미확정, 제품 미구현**. 2026-10-07.
기준: Windows 시험판 source f0ecfbd와 Monitor/Artifact 명세.
원본 작업 트리의 Artifact scope 변경은 수정하거나 완료된 것으로 가정하지 않는다.

**2026-10-13 현재 상태:** 위 날짜와 아래 수치·시험 계획은 최초 제안 당시 기록이다.
불변 Report 원칙은 유지한다. 이후 library 저장/읽기 구현은
[저장 계약 장부](GIL_Visual_Report_Storage_Contract_v0.1.md), 다음 화면 구현은
[표시 계약안](GIL_Visual_Report_Display_Contract_v0.1.md)을 기준으로 한다.
첫 표시 대상은 정적 PNG만이며 encoded 최대 16 MiB다. 아래 PNG/JPEG·8 MiB 제안을
현재 지원 범위로 읽지 않는다. 실제 Host 이미지 표시는 아직 검증하지 않았다.

## 1. 목표와 관측 근거

계산기를 만든 Verify 노드를 누르면 당시 화면과 설명이 함께 보인다. 이후 공학용 계산기로
수정하거나 revisit하더라도 이전 노드는 이전 화면을 보여 준다. 그림은 Report의 설명을
보조하며 성공 판정이나 실제 관측을 대신하지 않는다.

현재 코드:

- `src/report.rs`: Report는 이름과 문자열의 맵이다. 독자 문법이 값을 원문으로 보존한다.
- `src/monitor/detail.rs`: 선택한 Step의 저장 Report를 원문 필드로 투영한다.
- `ui/companion.js:renderDetail`: 값은 `textContent`; markup으로 해석하지 않는다.
- `src/mcp/monitor.rs`: 명시적으로 연 Project의 scope를 검증한 뒤 read-only 상세를 반환한다.
- Monitor Model §7.2: schema 합의 전 이미지/HTML/링크 자동 해석 금지.
- Artifact Model §5–6: Snapshot 확정과 작업 파일 변경의 허용 경계가 있다.

따라서 Markdown 이미지 문법을 렌더링하는 작은 UI 수정만으로 완료할 수 없다.

## 2. 대안과 권고

| 방식 | 장점 | 핵심 문제 |
| --- | --- | --- |
| 현재 파일의 상대경로를 읽기 | 구현이 작음 | 파일 교체/restore 뒤 과거 증거가 바뀜 |
| 기존 Snapshot + entry 참조 | 기존 불변 객체 활용 | Snapshot에 없는 캡처, Interview 자료를 처리하지 못함 |
| Report에 연결된 불변 evidence 객체 | 모든 Report에서 같은 자료를 재조회 | 저장·수명·호환성 계약 추가가 필요 |

세 번째를 기반으로 하되 자료 생성과 참조를 분리한다. 이미지 저장이나 생성 코드 수정은
**Verify에서 일어나는 세계의 변경**이다. 추적 범위 안의 자료는 기존 Artifact 확정 규칙을 따른다.
보고서의 불변 참조는 자료의 확정본을 보존하며 별도의 세계 변경 권한을 만들지 않는다.
하나의 자료를 여러 Report에서 재사용할 수 있다. 객체 저장 재사용은 소유·참조 유지 계약에서 정한다.

GIL이 보관할 가치가 있는 파일을 대신 선택하지 않는다. 사용자 요청/위임 안에서 Agent가
첨부할 자료와 목적을 설명하고 기록한다. 이미지 형식 제한은 화면 디코딩의 안전 경계이며
Artifact 추적 대상의 자동 제외 규칙이 아니다. Git ignore/추적 여부도 첨부 권한이 아니다.

## 3. 제안하는 흐름

1. 사용자가 고른 Project의 실험에서 필요한 자료의 목적·출처를 확인한다.
2. Verify에서 캡처/이미지를 저장하거나 생성 코드를 수정한다. 다른 Step은 확정 자료를
   참조할 수 있지만 Verify 밖의 파일 쓰기는 허용하지 않는다. 프로젝트 밖 임시 캡처를
   읽는 별도 기능은 첫 조각에서 제외한다.
3. 명시된 Project 상대경로 하나만 읽고, 파일 종류·크기·디코딩·범위를 검증한다.
4. Verify Report close 때 검증된 bytes의 불변 참조와 출처를 함께 확정한다.
   이후 다른 Step/Cycle Report close 때 이미 확정된 자료의 참조를 기록할 수 있다.
5. Monitor는 선택한 Step에 실제 연결된 자료만 읽는다. 자료가 없거나 손상돼도 텍스트 Report는
   남기고 별도 상태를 보여 준다. 최신 파일이나 다른 프로젝트의 동명 파일로 대체하지 않는다.

닫힌 Report에는 조용한 수정뿐 아니라 명시적인 사후 첨부·보충도 허용하지 않는다.
별도 supplement 객체나 Monitor 전용 첨부 테이블로 이 불변식을 우회하지 않는다.
시각 자료가 누락됐다면 과거 Report는 누락된 그대로 보존한다. 새 실행·관측에서 만든 자료는
새 Report에 속하며, 과거 실행 때 이미 존재했던 증거처럼 표시하지 않는다.
열린 Cycle을 둔 채 다른 Cycle을 여는 예외도 만들지 않는다.

### 3.1 Verify의 필수 선언과 누락 처리

모든 새 Verify Report는 시각 자료를 원칙적으로 첨부한다. 필요 없는 실험이면 **없음과 이유를
명시**한다. 조용한 생략은 허용하지 않는다. 필요하지만 생성 실패·누락으로 없는 경우도 사실과
이유를 명시하며 "불필요"로 위장하지 않는다. typed 상태·필드 이름은 저장/도구 계약에서 정한다.
기존 Report에 소급 삽입하지 않으며 구형 기록의 미기재와 명시적 없음을 구분한다.

사용자가 누락된 자료나 추가 시각화를 원하면 현재 Cycle을 정상 절차로 종료한 뒤,
**되돌아감 없이 후속 Cycle을 연다.** 그 Cycle의 Verify에서 시각 자료만 세계에 추가하는 것도
유효한 실험이다. 기존 기능을 의미 없이 재구현할 필요는 없다. 목표·가설·검증·분석·결과 절차는
유지하며 기존 Report를 덮어쓰지 않는다. 열린 Cycle을 두고 후속 Cycle을 미리 열지 않는다.

실험의 성공 조건을 충족했다면 추가 시각화 요청만으로 성공을 실패로 바꾸지 않는다.
반대로 실제 실패나 미충족 필수 성공 조건을 후속 Cycle 개설 편의 때문에 성공으로 꾸미지
않는다. 이때는 기존 실패 전이를 따른다. 성공 Cycle 직접 revisit 확장은 하지 않는다.

### 3.2 하나의 자료, 여러 보고서

Verify에서 확정한 자료는 같은 Cycle의 Analysis·Outcome·최종 Cycle Report와 다음 Cycle의
Define 등에서 재사용할 수 있다. 생성 출처는 원래 Verify로 고정하고 각 Report에는 참조 목적과
설명을 별도로 기록한다. 기존 자료 참조는 새 파일 생성이 아니다. 재가공·재캡처·내용 변경은
새 Verify의 새 자료로 확정한다. 재사용을 후속 실험에서 새로 관측한 증거로 표시하지 않는다.

예: C1 Verify에서 A 확정 → C1 최종 Report가 A 참조 → C2 Define에서 A를 개선 전 상태로
참조 → C2 Verify에서 B 확정 → 최종 Report에서 A/B 비교. 과거 Report는 바뀌지 않는다.

## 4. 제안 메타데이터 — 아직 wire schema가 아님

- 불변 evidence 식별자, 콘텐츠 digest, 실제 MIME, bytes 및 decoded dimensions
- Project 소유, 생성 Verify 출처, 여러 Step/Cycle Report의 참조 연결과 표시 순서
- caption, alt text, 출처 설명
- `captured` / `generated` / `provided` 구분
- 비교용 before/after 관계는 선택 사항이며 순서나 성공을 자동 추론하지 않음

captured는 캡처 자료라는 주장이지 서버가 화면 관측의 진위를 인증한다는 뜻이 아니다.
생성된 예상 화면은 `generated`로 표시하고 실행 결과로 보고하지 않는다.
UI에는 절대경로나 내부 object-store 경로를 보내지 않는다. digest도 접근 권한을 대신하지 않는다.

## 5. 저장과 복원 불변식

- Report 참조는 파일의 미래 내용이 아닌 등록 당시 bytes를 가리킨다.
- World restore/revisit가 확정된 Report와 첨부물을 지우거나 되감지 않는다.
- Node close 실패/취소 시 첨부 참조만 부분 확정하지 않는다. 재시도는 중복 연결을 만들지 않는다.
- crash 중 미참조 임시 객체가 남을 수 있다면 그 회수 기준을 Storage Model에 정의한다.
- 초기 버전은 확정 Report에 연결된 객체를 GC하지 않는다. 용량 부족 시 새 쓰기를 거절한다.
- 같은 bytes는 중복 저장을 줄일 수 있으나 Project 간 공유/검색/권한 우회는 하지 않는다.
- Project 이동·재시작 후 scope를 다시 검증하고 원래 자료를 읽는다.
- GitHub 공유, 업로드, 원격 백업은 자동으로 하지 않는다.

민감 자료는 저장 전에 가리는 것을 기본 안내로 한다. metadata 제거/재인코딩도 자료 변경이므로
조용히 수행하지 않고 가공본임을 기록한다. 이미 저장한 자료의 삭제/백업 잔존/원격 공유 철회는
별도 정책이 필요하다. 현재 단계에서 '영구 삭제됨'을 약속하지 않는다.

## 6. 최소 제품 범위와 보안

첫 조각은 PNG/JPEG 정지 이미지와 설명이다. 차트·시각화는 그 화면의 정지 이미지로 첨부한다.
SVG/HTML/PDF/동영상/실행형 차트와 외부 URL 자동 로딩은 제외한다. 이후 별도 검토한다.

제안 한도: 파일 8 MiB, decoded 16 megapixels, 한 노드 8개/합계 32 MiB. 이 수치는
미확정이며 실제 Host 메시지 제한과 메모리 시험 후 조정한다. base64의 증가분까지 계산한다.
단순 magic-byte 검사만 통과시켜 브라우저에 보내지 말고, bounded decoder의 성공을 확인한다.

- 경로 traversal, 절대경로, UNC/device/ADS, symlink/junction/reparse point, 범위 밖 경로 거절.
- Windows case/Unicode 정규화와 검사 후 파일 교체(TOCTOU), hardlink 별도 시험.
- 읽는 도중 크기/내용 변화, 제한 초과, 디코딩 폭탄, 위장 MIME/확장자, 손상 파일 거절.
- App 요청은 scope + Report 소유 노드 + 그 Report에 연결된 evidence ID로 한정한다.
  생성 Verify와 열람 Report가 달라도 명시된 참조를 검증한다. 임의 경로/hash 조회 API 금지.
- 원문 Report/caption/alt는 계속 textContent. 외부 HTTP/file URL 접근과 HTML 삽입 없음.
- 큰 bytes는 전체 그래프/매 poll에 싣지 않는다. 선택한 상세에서만 bounded 요청한다.
- blob URL을 쓴다면 검증된 bytes로만 만들고 선택 변경/화면 종료 때 해제한다.
- CSP는 필요한 image source만 좁게 추가한다. `img-src *`, `connect-src *` 금지.
- 사용자 로컬 자료가 Host에 전달되는 사실은 설명한다. 모델 컨텍스트 전달과 공개 공유는
  동일하지 않으며, App 전용 결과도 Host로의 전송이라는 점을 숨기지 않는다.

## 7. UX와 호환성

텍스트 보고서 아래에 썸네일·설명·출처·자료 종류를 둔다. 확대는 사용자 클릭으로 한다.
텍스트만 있는 기존 노드는 그대로 표시한다. 자료가 없는 상태와 로딩 실패를 구분한다.
빠른 노드 전환 때 늦은 응답을 이전 노드에만 귀속시키며 현재 상세에 끼워 넣지 않는다.
화면 하단의 채팅 겹침 여백과 fullscreen/가로 그래프 기본값을 유지한다.

format 4의 unknown field가 무시된다는 사실만으로 저장 호환을 주장하지 않는다.
구형 writer가 첨부 참조를 지우지 않도록 reader/writer 호환성과 거절 조건을 먼저 정한다.
새 도구/인자/action surface 및 storage format 변경 여부는 구현 전에 명세로 합의한다.
기존 문자열 필드에 특별한 접두사를 몰래 넣어 이미지 권한을 만드는 방식은 사용하지 않는다.

## 8. 인수 시나리오 — 모두 미실행

| 묶음 | 시나리오 | 합격 조건 |
| --- | --- | --- |
| 기본 | PNG/JPEG, 한글 이름, 여러 이미지, 텍스트만 있는 구형 노드 | 실제 자료와 설명, 명시된 순서 |
| 시간 | 같은 경로 덮어쓰기·원본 삭제·revisit·restore·재시작 | 과거 노드의 고정본 불변 |
| 의미 | 실행 캡처와 생성된 목업을 함께 첨부 | 서로 구분, 생성물을 관측으로 표시하지 않음 |
| 권한 | 다른 Project/Step ID, 임의 hash, 미등록 scope | bytes 반환 없이 거절 |
| 경로 | ../, 절대경로, UNC, ADS, junction, symlink, hardlink | 범위 밖 자료 읽지 않음 |
| 경쟁 | 검사 후 교체, 읽기 중 변경, 동시 close | 혼합 bytes/부분 Report 없음 |
| 한도 | 크기/픽셀/개수 경계값, 폭탄, 잘못된 MIME, 손상 | bounded 실패, Report 보존 |
| 주입 | caption HTML, SVG script, 외부 URL, file URL | 실행·외부 요청 없음 |
| 복구 | disk full, crash before/after close, 재시도 | 논리 상태 원자성·중복 연결 없음 |
| 화면 | 빠른 선택, stale 응답, fullscreen, 하단 채팅 | 다른 노드 자료 혼입/카드 가림 없음 |
| 호환 | 구형 Project/Host/writer, Project 이동 | 명시적 지원/거절, 참조 조용한 유실 없음 |
| 프라이버시 | EXIF/개인 화면, Git push/공유 요청 | 가공·전송·공개 경계 명시, 자동 공개 없음 |
| 불변성 | 닫힌 Report에 첨부 추가·교체·보충 시도 | 과거 Report·참조 불변, 우회 쓰기 API 없음 |
| 전이 | 열린 Cycle에서 새 Cycle, 성공 Cycle 직접 revisit 시도 | 기존 typed 거절 유지, 과거 판정 변경 없음 |
| 후속 작업 | 성공 Cycle 종료 후 시각화 전용 Cycle의 Verify에서 자료 생성 | 되돌아감 없이 세계에 추가, 과거 Report 보존 |
| 필수 선언 | 첨부 있음·불필요·필요하지만 누락·선언 생략·구형 기록 | 상태 구분, 새 Report의 조용한 생략 거절, 구형 기록 불변 |
| 재사용 | Verify 자료를 최종 Cycle Report와 다음 Define에서 참조 | 동일 확정본·생성 출처 유지, 참조 설명은 보고서별 |
| 출처 | 새 실행 캡처를 과거 실행의 자료로 연결 시도 | 새 Report에만 연결, 과거 관측으로 오인시키지 않음 |

## 9. 다음 결정과 구현 순서

승인된 원칙: **Report close 때 자료를 함께 확정하고 World restore 뒤에도 보존한다.
닫힌 Report의 보충 경로는 만들지 않으며, 누락은 정당한 새 작업과 Report로 남긴다.**

다음 구현 게이트:

[저장·공유 참조 계약안](GIL_Visual_Report_Storage_Contract_v0.1.md)에 필수 선언,
자료/참조 분리, 원자 close, format 이관 제안과 단계별 시험을 구체화했다. 아직 제품 미구현이다.

1. §3.1–3.2의 Verify 필수 선언·후속 Cycle·공유 참조를 계약으로 구체화한다.
   성공 Cycle 직접 revisit 확장 논의는 닫는다. 기존 전이를 유지한다.
2. typed 첨부 소유권과 저장/호환성 계약을 확정한다. 기존 format 4 자동 변환이나 구형 writer의
   참조 유실을 허용하지 않는다. 저장 형식 변경·마이그레이션은 별도로 승인받는다.
3. Core close의 원자 등록/범위 검증과 실패·복구 시험을 먼저 구현한다.
4. App 전용 읽기 → 상세 UI → Skill의 close 전 안내 → macOS/Windows 실측 순서로 진행한다.

이번 설계 변경은 실행 코드·저장 형식·설치본을 변경하지 않는다. §8은 시험 계획이지 통과 장부가 아니다.
완료되지 않은 저장소 변경과 Artifact scope 작업은 별도 브랜치에서 조율한다.

Windows 현황(사용자 보고): 외부 사용자 두 명이 계산기→공학용 계산기와 노드/카드 표시 성공.
두 사람 각각의 fullscreen 및 재시작 복원은 이 보고로 통과 처리하지 않는다.
