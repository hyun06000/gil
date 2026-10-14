# T1.2 — 보고서 이미지 표시 계약안 v0.1

작성: 2026-10-13. 상태: **G1–G3 개발 후보 구현, G4 Codex 이미지·설명·출처 표시 확인**.
채팅 동시 사용 지속성 등 G4 전체 검수와 G5 제품화는 미완이다. 아래 도구명·전송 한도는
개발 후보 계약이며 현재 배포 설치본에 적용됐다는 뜻이 아니다.

G1 구현 체크포인트(2026-10-13): `visual_report::display`에 읽기 전용 Project 투영을 추가했다.
Step/Cycle owner, 선언 상태, 순서 있는 참조·출처·문자열 asset ID를 반환한다. 이는 로컬 DTO이며
scope 검증·전체 응답 한도·도구 등록은 G2에 남는다. 저장 메타데이터 투영은 객체 bytes의
존재/디코딩 검증을 대신하지 않는다. 이미지 읽기는 기존 `read_report_image` 경계를 사용한다.
합성 PNG close→참조→원본 삭제→read-only 재개, Analysis/Outcome/Cycle/다음 Define 공유,
미기재/열림/없음/부분 누락·목록 순서를 시험한다. G3/G4 화면 구현·관측은 아직 없다.
검증: `cargo test --locked -p gil` 33개 결과 그룹 합계 1,299 통과/0 실패,
`cargo build --locked -p gil --all-targets` 통과, `git diff --check` 정상.
첫 전체 시험은 sandbox의 localhost bind 차단으로 lib 430 통과/50 실패 후 중단됐다.
로컬 포트 권한을 허용한 재실행이 위 전체 통과 결과다. 새 회귀시험은 5개이며
`visual_attachment` 22개에 포함된다. JS/Host/Windows 실측은 이번 변경에서 실행하지 않았다.

G2 구현 체크포인트(2026-10-13): App-only `gil_monitor_report_visuals`와
`gil_monitor_report_image`를 등록했다(총 19개). 엄격한 owner/asset/offset 입력,
Report 연결 권한, 각 요청의 저장 객체 검증, 최대 256 KiB chunk를 적용했다.
목록 JSON뿐 아니라 text 복제분을 포함한 전체 CallToolResult도 1 MiB로 제한한다
(JSON-RPC envelope는 제외). 승인 scope 복원은 기존 binding 경계를 사용한다.
실제 stdio에서 1 MiB 이상 합성 PNG를 조각으로 재조립하고 원본 byte 일치, 원본 삭제와
서버 재시작 후 재조회, 참조 없는 Report/미등록 scope/손상 객체/잘못된 입력 거절을 검사한다.
목록은 객체를 읽지 않으므로 이미지 유효성을 보증하지 않는다. 현재 안전한 저장 읽기 API가
형식·손상·미연결 자료를 동일한 `visual_unavailable`로 감추므로 아래 `unsupported_visual`은
후속 오류 세분화 제안으로 남긴다. Host 이미지/Blob 표시·CSP 동작·클라이언트 재조립은 G3/G4다.
G2 검증: Core 1,302 통과/0 실패(33개 결과 그룹), JS 86 통과/기존 4 skip,
패키징 및 Windows inventory 84 통과, all-targets 빌드·whitespace 검사 통과.
첫 MCP 파일 실행은 신규 이미지 시험이 통과했으나 기존 OS watcher hint 시험이 30초 만에
도착하지 않아 17 통과/1 실패였다. 권한을 허용한 전체 재실행에서는 통과했다. 첫 고지 검사도
새 base64 package로 인한 기대 개수 차이로 실패했고, 잠긴 의존성·원문 고지 검토와 개수 갱신 후
재실행했다. 실제 Host 렌더링·Windows 실행·설치본 교체·공개 배포는 하지 않았다.

기준: [제품 원칙](GIL_Visual_Report_Proposal_v0.1.md),
[저장 계약과 구현 장부](GIL_Visual_Report_Storage_Contract_v0.1.md),
[로드맵 T1.2](GIL_Roadmap.md).

## 1. 이번 작업의 끝

사용자가 Verify 노드를 선택하면 **당시 확정한 PNG와 보고서 설명**을 함께 본다.
원본 작업 파일을 변경·삭제하거나 Monitor/server를 재시작해도 같은 확정 bytes를 읽는다.
같은 이미지를 Outcome·최종 Cycle Report·다음 Define에서 참조할 수 있으며 원래 Verify
출처는 바뀌지 않는다. 재사용을 새 촬영·새 검증으로 보고하지 않는다.

첫 수직 검증 범위는 다음으로 제한한다.

`합성 format-5 Project → Verify 첨부 확정 → Report 참조 목록 → 제한된 이미지 조회 → 노드 상세 표시 → 재조회`

기존 사용자 Project 자동 이관, 공개 첨부 CLI/MCP 쓰기 도구, 일반 파일 브라우저,
임의 Markdown 이미지 해석, HTML/SVG 실행, 외부 이미지 다운로드, Companion 설치,
배포·게시·Git push는 이번 범위가 아니다.

## 2. 바꾸지 않는 원칙

- 새 Verify Report에는 `attached`, `not_needed` 또는 `missing` 선언이 반드시 있다.
  `not_needed`는 이유, `missing`은 이유·후속 계획을 가진다. 일부 첨부와 missing은 공존한다.
- 이미지 생성·재가공 등 세계 변경은 Verify에서 한다. 생성 코드만으로 첨부 완료가 되지 않는다.
- close가 Report·자료·참조를 함께 확정한다. 닫힌 Report의 사후 첨부·수정·삭제를 만들지 않는다.
- 추가 시각화는 성공 조건을 충족한 현재 Cycle을 정상 종료한 뒤 후속 Cycle의 Verify에서 한다.
  시각화 추가만을 위해 성공을 실패로 바꾸거나 되돌아가지 않는다. 열린 Cycle은 항상 하나다.
- 자료의 보관 가치와 추적 범위는 사용자 결정이다. PNG 제한은 안전한 표시 형식의 제한이지
  다른 파일을 세계 기록에서 임의로 제외하는 규칙이 아니다.
- 과거 선언 없는 Report를 `not_needed`로 조작하지 않는다.

## 3. 현재 구현과 필요한 작업

| 경계 | 현재 개발 체크아웃 | 다음 구현 |
| --- | --- | --- |
| 불변 자료/참조 | registry·format 5·실험용 Session close API 있음 | 합성 fixture로 종단 연결 |
| 안전한 읽기 | PNG 검증, Report 제한 읽기, 내부 `report_image` 있음 | 공개 조회 계약과 오류 변환 |
| 상세 | `NodeDetailV1`은 Step의 텍스트 Report만 반환 | 별도 시각 참조 DTO, Cycle Report 선택 경로 |
| 전송 | 이미지 tool/resource 미노출 | App 조회 도구·schema·manifest·계약 시험 |
| 화면 | 텍스트 상세와 fullscreen 있음 | 이미지·설명·출처·오류 상태 |
| 사용자 이관 | 기본 format 4, 실험용 opt-in만 있음 | 이번 구현에 포함하지 않음 |

조회는 `ReadOnlySession`을 사용한다. 이미지 보기 때문에 Snapshot·Report·선택한 세계를
저장하거나 rollback/cleanup을 실행해서는 안 된다. 남은 범용 transaction 준비/전환 보호는
공통 보안 트랙이다. 다만 이미지 경로에서 확인한 권한·무결성 위험은 노출 전에 해결한다.

## 4. Report 참조 목록 — 제안 wire 계약

기존 `gil_monitor_detail`/`NodeDetailV1`을 재해석하지 않고,
새 읽기 전용 `gil_monitor_report_visuals`를 제안한다.

입력은 `scope_id`와 `owner`만 받는다. owner는 다음 둘 중 하나이며 canonical ref를 검증한다.

```json
{"scope_id":"project:…","owner":{"kind":"step","ref":"step:C1/S3"}}
```

Cycle Report는 `{"kind":"cycle","ref":"cycle:C1"}`다. 클라이언트가 주장한 단계 종류나
Report 존재 여부를 믿지 않고 실제 Project graph에서 확인한다.

응답은 `schema_version: 1`, 요청과 동일한 `scope_id`, `owner`, `state`, `uses`를 가진다.
reason/follow_up은 해당 상태에서만 제공한다. 알 수 없는 owner는 오류이며 빈 보고서가 아니다.

| state | 의미/화면 문구 |
| --- | --- |
| `open_report` | 아직 확정되지 않은 보고서입니다 — 첨부 0건으로 완료 판정하지 않음 |
| `legacy_unspecified` | 당시 시각 자료 선언이 기록되지 않았습니다 |
| `not_needed` | 시각 자료 없음 — 저장된 이유 표시 |
| `missing` | 시각 자료 미완 — 이유·후속 계획과 존재하는 첨부 표시 |
| `attached` | Verify의 확정 첨부 표시 |
| `references` | 다른 Step/Cycle Report의 인용 목록; 빈 목록은 인용 없음 |

`uses`는 저장된 순서를 지킨다. 각 항목에는 다음을 포함한다.

- `asset_id`: 양의 정수 ID를 **문자열**로 전달; JavaScript Number 변환 금지.
- `caption`, `alt`, `purpose`: 이 Report에 저장된 사용 설명, 원문 보존.
- `origin_step_ref`: 처음 자료를 확정한 Verify.
- `source_kind`, `source_description`: captured/generated/provided라는 작성자의 출처 주장.
- `mime_type`, `byte_length`, `width`, `height`, `sha256`: 검증된 객체 메타데이터.

절대 파일 경로·로컬 설정·저장 객체 경로는 응답에 넣지 않는다. 출처 문구가 실제 화면을
관측했다는 인증은 아니다. 설명이 비어 있는 오래된/비정상 자료에 관측 내용을 지어내지 않는다.
목록은 저장 계층의 크기 제한 안에서 반환하고 별도 응답 상한을 1 MiB로 둔다. 초과는
`response_too_large`로 명시한다. 조용한 잘라내기나 미표시 자료를 ‘없음’으로 처리하지 않는다.

## 5. 이미지 전송 — 제한된 App 조회

첫 구현 후보는 App 전용 읽기 도구 `gil_monitor_report_image`다. 현재 Host bridge의
`callServerTool` 경로를 사용하되 **실제 Host 검증 전에는 호환성을 확정하지 않는다**.
App 전용 메타데이터는 서버 권한 검사를 대체하지 않는다.

입력: `scope_id`, `owner`, `asset_id`, `offset`(0부터 시작하는 byte 정수).
호출자가 파일 경로·URL·digest·MIME·길이를 지정할 수 없다.

응답: `schema_version:1`, scope/owner/asset, `sha256`, `mime_type:image/png`,
`total_bytes`, `offset`, `next_offset`(마지막은 null), `data_base64`.
한 응답은 원본 최대 **256 KiB**의 chunk만 담는다. 임의 요청 길이는 받지 않는다.
offset은 256 KiB 경계여야 하고 파일 길이 밖이면 거절한다. 객체 최대 16 MiB는 유지한다.

매 호출에서 다음을 모두 검사한다.

1. 명시적으로 연 Project의 유효한 scope와 현재 Project identity.
2. 실제 닫힌 Report에 해당 asset 참조가 존재함.
3. registry에서 결정한 객체를 읽고 길이·digest·정적 PNG 검증.
4. 응답 직전 scope/Project identity 재확인.

초기 구현은 요청마다 제한된 객체 재검증을 허용하며 영구 bytes cache를 만들지 않는다.
최적화는 권한·무결성·메모리 한도를 보존하는 별도 변경으로 한다.
App은 모든 chunk의 identity/digest/offset/총길이가 같고 완결됐는지 검증한 뒤 digest를
확인하여 Blob을 만든다. 누락·중복·순서 오류·다른 객체 혼합은 표시하지 않는다.

기존 이미지 제한: 정적 PNG, encoded 16 MiB, 한 변 8192, 총 16,777,216 pixels,
decoded 128 MiB. APNG·손상 PNG·SVG·HTML·JPEG는 이번 표시 경로에서 거절한다.
첨부 시 최대 8개/합계 32 MiB와 조회 chunk 한도는 서로 다른 제한이다.
무손실 원본을 몰래 축소·재인코딩하지 않는다. 미리보기 파생물은 후속 별도 계약이다.

Host가 App 전용 조회/Blob 표시를 지원하지 않으면 텍스트 Report와 미지원 안내를 유지한다.
로컬 HTTP server, 임의 file URL, 외부 업로드, 전역 CSP 완화로 우회하지 않는다.
정확한 CSP/Host 정책은 G2·G4에서 검사하며 필요한 변경은 좁게 검토한다.

## 6. 화면과 오류

- 이미지 클릭은 상세 내부에서 원본 픽셀 크기로 전환한다. 별도 확대/축소 버튼은 없다.
  큰 이미지는 제한된 뷰포트 안에서 양방향 스크롤한다. 다시 클릭·Esc로
  미리보기에 돌아온다. 키보드 접근과 포커스 복귀를 제공한다. 기존 검증된 단일 이미지
  요소를 재사용하고 추가 이미지 요청·복사·저장·외부 창·CSP 변경은 하지 않는다.
  노드/자료/Project 전환·연결 종료 시 기존 release 경로와 함께 확대 상태도 폐기한다.

- 노드 선택 즉시 텍스트를 유지하고 시각 자료 목록을 별도로 읽는다. 가로 DAG·fullscreen
  기본값·사용자의 inline 선택·하단 채팅 안전 여백은 바꾸지 않는다.
- 상세의 ‘시각 자료’ 영역에 이미지, caption, purpose, 출처 종류, 원래 Verify를 표시한다.
  alt는 접근성 텍스트로 사용한다. 모든 설명은 textContent이며 HTML로 해석하지 않는다.
- 여러 첨부는 보고서 순서대로 이미지·설명·출처를 함께 표시한다. ‘자료 1/2/3’ 선택 버튼은 없다.
  한 번에 하나씩 로드/디코딩하며 보고서당 최대 8개를 유지한다. 각 원본은 최대 16 MiB,
  각 디코딩 이미지는 최대 16,777,216픽셀이다. 이전의 단일 이미지 메모리 상한은 적용되지
  않는다. 브라우저 내부 메모리 사용량을 보장한다고 주장하지 않는다.
- 선택·Project·연결이 바뀌면 요청 epoch를 바꾸고 이전 응답을 버린다. Blob URL은 교체·닫기·
  연결 해제 시 revoke한다. persistent browser cache/localStorage에 이미지 bytes를 저장하지 않는다.
- loading / ready / unavailable / unsupported / retryable error를 구분한다. broken-image 아이콘만
  남기지 않고 ‘이미지를 읽지 못했습니다. 보고서 본문은 보존되어 있습니다’와 짧은 이유를 보인다.
- 일시적 busy/연결 오류만 자동 1회 재시도한다. 무권한·무결성·미지원·Project 이동에는 반복하지
  않는다. 사용자의 다시 시도도 동일한 권한 검사를 거친다.
- Cycle 최종 Report는 기존 Step에 위장하지 않는다. Cycle 영역에 명시적인 ‘Cycle 보고서’
  진입점을 추가하여 같은 owner 계약을 사용한다. 접기/펼치기 제스처와 충돌하지 않게 한다.
- 재시작 후 현재 승인된 scope로 다시 조회한다. 이전 화면의 asset ID만으로 다른 Project를
  열지 않는다. 선택 상태의 완벽한 복원은 별도 UX이며 같은 자료 재조회와 구분한다.

외부 오류는 기존 scope 거절을 유지하고, 자료 없음/손상/연결 불일치에는 경로 없는
`visual_unavailable`을 사용한다. malformed offset은 `invalid_request`, 지원 밖 형식은
`unsupported_visual`로 제안한다. 응답은 민감한 경로나 bytes를 로그에 남기지 않는다.

## 7. 정보 전달과 사용자 데이터

### Cycle 보고서 선택 세부 계약 (2026-10-14)

그래프의 Cycle 사각형을 눌러 보고서를 고른다. 별도 드롭다운은 없다. 사각형은 키보드로도
선택할 수 있고 Step 원형 노드보다 뒤에 놓인다. 접기/펼치기는 기존 별도 조작을 유지한다.
접힌 Cycle은 먼저 펼쳐 사각형을 선택한다. 선택하면 Step 선택을 해제하고 Cycle 요약 카드와 기존
MonitorViewV1의 Cycle Report 투영(판정·인계·Outcome 교훈·당시 다음 방향)을 표시한다.
‘보고서 요약’임을 밝히며 전체 원문이라고 부르지 않는다. 교훈은 Outcome에서 온 값임을
표시한다. 열린 Cycle에는 아직 Report가 없다는 안내를 유지한다.

이미지는 정확한 `{kind:cycle, ref}`로 기존 App-only 조회 경계를 사용한다. Step 선택 시
Cycle 선택과 이미지가 해제된다. 선택 경쟁·Project 전환·없는 Cycle·재조회 시 이전 owner
응답을 표시하지 않는다. 재조회 성공은 현재 Cycle 선택을 유지하고 없어진 선택은 비운다.
본문은 textContent이며 저장·닫기·세계 변경·자동 Cycle 열기 동작은 없다. 새 schema/도구나
암묵적 Step/Cycle 자료 탭은 추가하지 않는다. 재연결 후 선택의 완벽한 복원은 요구하지
않지만 같은 Cycle을 다시 선택해 같은 보고서와 자료를 읽을 수 있어야 한다.

Monitor 표시에는 로컬 서버→Host→App으로 이미지 bytes 전달이 필요하다. ‘로컬 파일이므로
Host에도 전달되지 않는다’고 약속하지 않는다. 이미지에는 화면·메타데이터 속 개인정보가
있을 수 있고 원본 보존은 개인정보 제거가 아니다.

선택한 보고서의 자료만 필요할 때 로드한다. 전체 Project를 미리 전송하지 않는다.
이미지를 모델용 multimodal content로 자동 첨부하거나 `sendMessage`로 대화에 보내지 않는다.
App-only 설정도 Host의 처리·보존 정책에 대한 기밀성 보장이 아니다. 외부 공유는 별도 사용자
의사결정이다. 기존 설치본/기존 자료는 이번 검증 중 교체하거나 이관하지 않는다.

## 8. 검수 순서와 통과 조건

| Gate | 시험 | 통과 증거 |
| --- | --- | --- |
| G1 저장→목록 | 격리된 합성 format-5 fixture, Verify PNG close와 참조 | 정해진 digest·원출처·Report별 설명 일치 |
| G2 실제 stdio | 목록/이미지 chunk schema·권한·오류·한도 | 실제 wire 응답, 미등록 scope/미연결 asset 거절 |
| G3 UI 자동시험 | fake bridge, PNG decode, 선택 경쟁, 오류, Cycle 상세 | DOM·alt·설명·출처·메모리 수명 시험; Host 성공 아님 |
| G4 실제 Host | 개발 후보에서 노드 선택→fullscreen 이미지 표시 | 사용자 화면 확인 + 전송/연결 진단; 텍스트 응답만으로 통과 금지 |
| G5 제품화 | 명시적 이관·첨부 쓰기 UX·회복·패키징·플랫폼 | 별도 명세/승인/시험 후 사용자 배포 판단 |

G1–G4의 필수 시나리오:

1. Verify 1 PNG, 여러 PNG 전환, 같은 asset을 Outcome/Cycle/다음 Define에서 서로 다른 설명으로 인용.
2. not_needed 이유, missing만 있음, 일부 첨부+missing, 열린 Report, 과거 미선언 Report.
3. 원본 세계 파일 변경·삭제 후 동일 이미지; server/App 재시작 후 동일 digest·출처.
4. 다른 Project ID, 실제 참조 없는 asset, 미승인 scope, Project 교체/이동, 중간 연결 해제.
5. 저장 객체 누락·digest 불일치·손상 PNG·지원 밖 형식·oversize·잘못된 offset/chunk 혼합.
6. 빠른 노드/Project 전환에서 이전 이미지가 새 보고서에 나타나지 않음; 오류에도 본문/그래프 유지.
7. caption/alt/출처의 HTML 같은 문자열이 실행되지 않음; 파일 경로·이미지 bytes가 로그에 없음.
8. 읽기 전후 Project 기록 bytes 불변; 이미지 조회가 restore/cleanup을 실행하지 않음.
9. format 4 일반 프로젝트는 기존 텍스트 상세 유지; 자동 format 5 전환 없음.
10. Codex macOS 실제 표시를 먼저 확인하고 Windows/Claude는 각각 별도 검수. Unix 시험이나
    cross-compile로 Windows ingest 안전성·Host 표시 성공을 대신하지 않음.

합성 검증에는 승인된 임시 fixture만 사용한다. 실제 사용자 Report를 변경해서 시험하지 않는다.
새 wire 도구가 추가되므로 tool 수·manifest·schema·identity 호환성 시험을 함께 갱신한다.
Agent의 첨부 작성 Skill과 공개 쓰기 도구는 G5에서 다루며 조회 시제품 성공으로 완료 처리하지 않는다.

## 9. 다음 구현 단위

먼저 G1과 Report 참조 DTO를 구현하고 시험한다. 그 다음 G2 전송, G3 화면, G4 설치 후보
검수 순으로 진행한다. transport 지원이 실제 Host에서 막히면 원인과 대안을 이 계약에 기록하고
구현 전에 검토한다. 범용 복원 작업으로 목표를 바꾸거나 사용자의 데이터를 이관해 우회하지 않는다.

## 10. G3 UI 구현 기록 (2026-10-13)

- 공용 renderer는 선택한 Step의 본문 아래 선택적 Host hook으로 시각 자료 영역을 붙인다.
  native/fixture Host에 hook이 없으면 기존 텍스트 상세를 그대로 유지한다.
- MCP App은 목록과 이미지 chunk를 읽고 scope/owner/asset/hash/offset/길이/PNG 크기를
  검증한 뒤 Blob URL을 만든다. 이미지·caption·alt·용도·원래 Verify·작성자가 기재한 출처를 표시한다.
- Step 자료와 Cycle 보고서 **시각 자료**를 전환할 수 있다. Cycle 텍스트 전체 상세 추가와는
  구분한다. 여러 자료는 한 번에 하나씩 읽는다. 열린/과거 미기재/없음/누락을 구분한다.
- epoch로 노드·자료 선택이 바뀐 응답을 버리고 조립을 직렬화한다. 교체·종료·연결 오류 시
  Blob URL을 해제한다. 이미지 오류는 본문/그래프를 지우거나 Companion을 실행하지 않는다.
- 새 회귀시험: 다중 chunk 완전 일치, 문자열 ID, 혼합 응답·범위 초과·잘못된 digest·stale 거절.
  모의 Host Chrome에서 합성 차트 decode, caption 문자 이스케이프, alt/출처, Cycle 자료,
  손상 응답, 지연 응답 뒤 노드 전환, 연결 실패 시 본문 유지와 복구를 확인했다.
- 기존 하단 여백 시험은 이미지 영역이 본문 아래에 추가되어 마지막 텍스트 필드가 화면 위로
  올라가는 바람에 첫 실행에서 실패했다. 스크롤 끝의 출처 마지막 줄을 대상으로 보정하고,
  기존 32줄 본문이 잘리지 않았는지는 별도로 유지해 세 viewport에서 통과했다.

실제 Codex/Claude의 CSP·Blob 허용/표시를 검수한 것은 아니다. 공개 설치본 교체·배포·이관은
하지 않았다. G4에서는 합성 format-5 후보를 실제 Host에 연결해 이미지와 채팅을 함께 확인한다.

최종 G3 검증: JS 90 통과/기존 4 skip, 패키징·Windows 고지 inventory 84 통과,
Rust 내장 surface 시험 2 통과, all-targets 빌드 통과. 모의 Host Chrome 시험 및 공용
native/fixture UI selftest 70/70 통과. 이미지 선택 시 live Blob URL 1개, 연결 종료 시 0개도
확인했다. 전체 Core suite는 이번 UI 변경에서 재실행하지 않았다(직전 G2 결과 1,302 통과).
새 UI bundle hash에 맞춰 고지 입력 지문을 갱신했으며 의존성 목록은 변경하지 않았다.

## 11. G4 첫 실제 Codex 관측 (2026-10-13, 미통과)

승인된 별도 개발 MCP 연결과 합성 format-5 Project를 사용했다. 기존 설치 Plugin과 사용자
Project는 교체/이관하지 않았다. 실제 Codex MCP App의 DOM에서 펼친 Monitor 상태,
그래프·Verify 본문·시각 자료 목록의 자료 1/2 버튼은 확인했다. 그러나 figure는 비어 있었고
"이미지를 읽지 못했습니다"가 표시됐다. 이미지와 채팅 동시 표시 성공은 아직 아니다.

목록 이후 어느 단계가 실패했는지는 첫 관측으로 특정할 수 없다. 수신/검증/digest/Blob/PNG
decode 단계를 고정된 코드로만 표시하는 진단을 추가했다. 임의 Host 오류문·경로·bytes는
표시하지 않는다. SHA-256 기능 부재도 구분하되 무결성 검사를 생략하는 우회는 넣지 않았다.
관련 JS 시험 5건과 native 서버 빌드가 통과했다. 새 진단 bundle 적용은 임시 서버 재연결 후
다시 관측해야 한다. mock 시험 통과를 실제 Host 성공으로 대체하지 않는다.

### G4 비교 후보

실제 Host 재관측에서 두 합성 PNG 모두 `png_decode/failed`였다. 수신·조립·SHA-256 검증과
Blob URL 생성은 지난 상태이며, 이 결과만으로 CSP 차단 원인까지 단정하지 않는다.

비교 후보는 먼저 Blob PNG를 읽고 실패하면 **같은 검증된 Blob**을 FileReader로
`data:image/png;base64`로 변환하여 한 번 읽는다. 외부 URL·filesystem URL·추가 tool 요청이나
CSP 허용 목록 변경은 없다. SHA-256·크기·PNG 차원 검사와 선택 epoch/자원 해제는 유지한다.
자료당 실패한 Blob URL은 fallback 전에 폐기하며 data src도 교체/종료 시 제거한다.
실제 CSP violation 이벤트의 `img-src`/Blob 차단 여부만 고정 문구로 기록하고 원문 URL은
표시하지 않는다. 이것은 Host가 이미 허용한 표현만 사용하는 경로이며 둘 다 거절하면 중단한다.

모의 Chrome 비교에서 `img-src data:`는 Blob 차단 이벤트 + 동일 PNG data decode 성공,
`img-src 'none'`은 두 방식 실패 + image 0/Blob URL 0 + 본문 보존을 확인했다.
첫 비교시험은 차단 이벤트가 비동기로 도착하기 전에 진단 문구를 확정해 assertion이 실패했다.
data decode 완료 시점의 관측값을 읽도록 보정한 재실행은 통과했다. data-only 모의 Host
전체 회귀 및 공용 UI selftest 70/70도 통과했다. 실제 Codex 비교 결과는 재연결 뒤 확인해야 한다.

### G4 실제 Codex 이미지 표시 확인 (2026-10-13)

재연결한 비교 후보에서 실제 MCP App DOM과 스크린샷으로 Verify의 파란/초록 차트 전환,
설명·용도·작성자 기재 출처·원래 Verify C2/S3 표시를 확인했다. Analysis C2/S4로 이동한 뒤
같은 파란 차트와 다른 인용 설명, 유지된 원래 Verify 출처도 확인했다.
진단은 `Blob PNG: CSP img-src 차단 확인 · data PNG: 표시 성공 (동일 검증 bytes)`였다.
이 Host에서의 최초 실패 원인은 Blob img-src 차단으로 확인됐다. CSP 설정과 무결성 검사는
변경하지 않았다. App은 펼친 상태를 보고했다. 자동 관측 범위는 App 내부이므로 Host 채팅과
이미지의 동시 가시성·대화 지속 중 유지 여부는 사용자 확인 전이며 G4 전체 완료로 처리하지 않는다.

### 사용자 확인 및 보고서 표시 범위 정정 (2026-10-13)

사용자가 실제 시각 자료가 잘 보인다고 확인했다. 다만 이미지와 채팅 동시 사용의 지속성까지
별도로 확인한 것으로 확대하지 않는다. 사용자 요청에 따라 `이 Step 자료` / `Cycle 보고서 자료`
전환을 제거했다. 선택한 노드의 보고서가 사용하는 이미지·설명·출처만 그 본문 아래 표시한다.
같은 보고서에 여러 이미지가 있으면 자료 선택은 유지한다. 다른 보고서를 보기 위해 이 패널의
owner를 암묵적으로 Cycle로 바꾸지 않는다.

자료 재사용은 저장/참조 기능이며 UI 전환 버튼과 별개다. Report별 caption과 불변 원출처,
같은 asset을 Analysis/Outcome/Cycle/다음 Define에서 참조하는 Core API는 변경하지 않았다.
앞의 G3 구현 기록에 있는 두 전환 버튼 설명은 당시 구현 이력이며 현재 UI 계약이 아니다.

재연결 뒤 실제 Codex MCP App에서 두 전환 버튼이 각각 0개임을 확인했다. Verify의 이미지와
설명·출처, Analysis에서 재사용된 이미지의 별도 설명과 원래 Verify 출처를 확인했다.
다음 작성 경로는 [G5 작성 계약안](GIL_Visual_Report_Authoring_Contract_v0.1.md)을 따른다.

### 실제 크기 캡처 및 원본 크기 보기 후보 (2026-10-14)

합성 후속 Cycle C3 Verify에서 실제 Codex Monitor 캡처를 첨부했다. 화면 도구가 반환한
bytes는 JPEG여서 최초 PNG 입력이 거절됐다. 미확정과 열린 owner를 조회한 뒤 원본을
보존하고 PNG로 명시적 변환하여 첨부했다. 638×875, 209,990 bytes, SHA-256
`2a7f8cf696e8b0d9d53e1466bebdea9918d6074bcebac9d9cea8c26b8b67088c`다.
실제 노드에서는 약 415×569로 축소되어 작은 글씨가 읽기 어려웠다. 설명과 출처는 표시됐고
이미지를 눌러도 확대되지 않아 가독성은 부분 통과로 기록한다.

개선 후보는 단일 검증 이미지 요소를 재사용하는 상세 내부 원본 크기 토글이다. 이미지 클릭과
명시적 버튼, 양방향 스크롤, Esc 복귀·포커스를 제공한다. 서버 도구/저장 계약/CSP는 그대로다.
JS 95 통과/4 기존 skip, 패키징·고지 시험 81 통과, release 빌드·20개 도구·압축 왕복 통과.
최초 패키징은 UI 지문 변경에 따른 notice-policy 불일치로 멈췄다. 의존성 추가 없이 UI 입력
지문만 갱신하여 재실행했다. 전체 Rust suite와 브라우저 렌더링 회귀는 이번에 실행하지 않았다.
새 개발 패키지 아카이브 SHA-256은
`24bd5443fdf560cbe1509f377e0b644486c492a8f58bdc15888c935a87524058`이다.
임시 `gil-visual-authoring-check` 등록만 새 후보로 변경했으며 기존 설치본/보고서는 유지했다.
실제 Codex 확대·축소 비율, 스크롤, 노드 전환 시 해제는 재연결 후 검수해야 한다.

재연결 후 실제 Codex MCP App 검수: 이미지 클릭으로 약 414.69×568.74 미리보기에서
정확히 638×875 원본 크기로 전환됐다. 상세 내부 viewport는 610×656, scroll extent는
638×875로 양방향 overflow를 확인했다. 복귀 버튼과 이미지 재클릭 모두 원래 미리보기
크기로 돌아왔다. 확대 상태에서 Define 선택 시 이미지 0개와 해당 본문을 확인했다.
실제 캡처에서 제목/본문은 더 크게 읽히며 설명·용도·캡처 출처는 유지됐다. 원본에 이미
작게 찍힌 글씨 자체의 해상도를 높이는 기능은 아니다. 자동화가 키보드·휠을 제공하지 않아
실제 Host Esc/스크롤 입력은 미검수(상태 로직 단위시험과 구분). Host 채팅 동시 사용의
사람 확인도 별도다. 이번 턴에는 Project 보고서/자료를 쓰지 않았고 CSP도 바꾸지 않았다.

후속 사용자 검수(2026-10-14): 사용자가 “Esc와 스크롤도 잘 된다”고 확인했다.
위 자동화 검수에서 남겼던 실제 Host Esc/스크롤 입력 항목은 사용자 실측 통과로 갱신한다.
이 확인은 macOS Codex 임시 개발 패키지에 한정하며 Windows나 정식 설치본의 인수로
확장하지 않는다. 원본 해상도 자체를 높이지 않는다는 제한은 그대로다.

### Cycle 보고서 선택 개발 후보 검증 (2026-10-14)

명시적 읽기 전용 선택란과 canonical View 보고서 요약 표시를 구현했다. 이미지 요청은
선택한 Cycle owner만 사용한다. Step 선택과 상호 배타적이며 늦은 Step 응답을 차단한다.
JS 97 통과/4 기존 skip, 공용 브라우저 selftest 72/72, 패키징·고지 81 통과다.
이전 단일 선택란 assertion은 첫 실행에 실패했고 Project와 Cycle 읽기 선택란만 허용하도록
보정했다. release 빌드·20개 도구·압축 왕복 통과. 개발 아카이브 SHA-256:
`0565f9cb778af2cb32af79494b4beb342170d0ebc122a63ad494d33fdb0ec9c2`.
독립 stdio 서버 종료·재기동에서 Verify/Analysis/Cycle 보고서와 동일 이미지 bytes,
원출처, `.gil` 8개 파일 지문 불변을 확인했다. 임시 MCP 등록만 갱신했다.
실제 Codex 연결 재시작과 새 Cycle 선택 화면 인수는 아직 대기다. 전체 Rust suite,
Windows, 기존 설치본 교체, 실제 사용자 기록 이관은 이번 검수 범위가 아니다.

후속 Host 검수(2026-10-14): 사용자 재연결 후 새 Monitor의 C3 선택에서 최종 보고서 요약,
638×875 PNG와 독립 설명·용도·captured 출처·원래 Verify C3/S3가 표시됐다.
원본 확대와 미리보기 복귀를 실제 App에서 확인했다. 이전 Report/이미지 지문과 같고
CSP는 변경하지 않았다. 위 재연결 후 선택 화면 대기는 이 범위에서 통과로 갱신한다.
자동 선택 복원과 Host 채팅 동시 가시성까지 App 내부 캡처로 증명하지 않는다.
