# 시각 증거 정책 인수 기록 (2026-11-02)

정책 정본: [spec/GIL_Visual_Evidence_Policy_v0.1.md](../spec/GIL_Visual_Evidence_Policy_v0.1.md).
아래는 **자동 테스트 통과 / 합성 실례 관측 / 사용자 화면 확인 대기**를 구분해 적는다. 도구 응답 성공을 화면
성공으로 세지 않는다.

## 1. 자동 테스트 (macOS arm64, Rust 1.97.1, Node 26)

| 묶음 | 명령 | 결과 |
| --- | --- | --- |
| Core lib (제품 프로필) | `cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib` | 686 통과 / 0 실패 / 1 ignored |
| Core 통합 (제품 프로필) | `… --test visual_attachment --test mcp --test manual --test monitor --test guidance_wave1 --test guidance_wave2 --test approved_plan_wire_wave1 --test context --test context_profiles` | 통과. `tests/manual.rs`·`tests/evidence.rs`의 4+4건은 approved-plan 프로필에서 기존 fixture(`bootstrap_from`의 맨 `approved: yes`, product-safety의 restore 차단)가 적용되지 않는 **기존 조건**이며 CI도 이 조합으로 그 파일을 돌리지 않는다 |
| Core 전체 (`visual-authoring`) | `cargo test --locked -p gil --features visual-authoring` | 전체 통과 (CI codex-preview visual-authoring 작업과 같은 조합) |
| Core 전체 (기본) | `cargo test --locked -p gil` | 전체 통과. 두 묶음 합계 3,070 통과 / 0 실패 |
| all-targets 빌드 | 기본, `approved-plan,visual-authoring,product-safety` | 통과 |
| JS | `node --test ui/layout.test.mjs mcp-app/*.test.mjs plugins/gil-companion-prototype/*.test.mjs` | 165건 중 161 통과 / 0 실패 / 기존 4 skip |
| 패키징 | `node --test distribution/codex/*.test.mjs distribution/compliance/notices.test.mjs` | monitor.html hash 갱신 후 102 통과 / 0 실패 / 기존 1 skip |
| 시나리오 채점기 | `node --test tools/scenarios/*.test.mjs` | 38 통과 / 0 실패 |
| Skill 동기화 | `node plugins/gil-companion-prototype/sync-workflow.mjs` | 일치 |

새 회귀시험: `visual_report::tests::not_needed_refuses_boilerplate_and_one_word_reasons`,
`image_evidence_is_recognised_by_extension_only`, manual 고정 순서 시험의 `report/visuals`, JS의 `visualCounts`·
`visualSummary`·plan node `visuals` 전달, 채점기 `visual_evidence_attached`. 기존 fixture 가운데 한마디 이유
(`문서 증거`)를 쓰던 stdio 시험 한 건은 구체적 이유로 바꿨다.

## 2. 합성 실례 — 요구 없는 시각 자료 (개발 빌드 `visual-authoring,product-safety`, format 5)

임시 폴더에 합성 `data/plant-growth.csv`(세 그룹 × 20)를 두고 MCP stdio로 Interview → Experiment를 완주했다.
개인 경로·대화는 포함하지 않는다. Verify에서 Python(matplotlib)으로 평균·표준편차 표(`evidence/group-means.md`)와
막대 그래프(`evidence/group-means.png`, 960×540)를 만들었다. Core 거절과 성공을 순서대로 관측했다.

| 시도 | 입력 | Core |
| --- | --- | --- |
| 1 | `not_needed`, reason `불필요`, 증거 md만 | 거절 `visual_reason_weak` — 닫히지 않음 |
| 2 | `not_needed`(구체적 이유)인데 evidence_files에 PNG 포함 | 거절 `visual_declaration_conflict` — 닫히지 않음 |
| 3 | `attached` + attachments/uses | 성공, asset_id 1, receipt에 sha256 |
| 4 | Analysis를 일반 텍스트 close | 거절 `[visual_citation_required]` — 인용 가능한 asset_id·sha256을 거절문에 포함 |
| 5 | Analysis typed close, stored source 인용 | 성공 (`references`, 1개) |
| 6 | Experiment Cycle Report typed close, stored source 인용 | 성공 (`references`, 1개) |

이 실례의 작성자는 workflow를 따르는 Agent 역할의 Claude(이 세션)였다. **모델이 지침만으로 스스로 그림을
만드는 비율**은 측정하지 않았다 — `mcp-app/guidance-eval.mjs`(5개 케이스)와 `interview-workflow-eval.mjs`
(`visualAttachedUnprompted`)가 그 측정 도구이며 유료 모델 호출이라 실행 전 모델·비용 확인이 필요하다.

## 3. Monitor 렌더링 — 개발용 mock Host (사용자 화면 확인 아님)

같은 Core 기록을 저장소의 공용 Monitor UI(`ui/` + `mcp-app/visuals.mjs`)로 렌더링하는 개발용 mock Host를
Claude 내장 브라우저에서 열어 관측했다. 캡처: [captures/visual-evidence-20261102/](captures/visual-evidence-20261102/).

- `graph-badges-mock-host.jpg`: Verify·Analysis 노드의 시각 자료 배지(1), 진행 패널의
  `닫힌 단계 9개 · … · 시각 자료 1개 (이 Cycle 1개)`.
- `verify-node-detail-mock-host.jpg`: 노드 상세에서 `시각 자료 1개` 패널이 Report 본문(evidence_files·execution·
  result)보다 **먼저** 서고, 플롯·caption·용도·출처(생성 · matplotlib … · 원래 Verify: step:C2/S3)·
  `Blob PNG: 표시 성공`이 보인다.
- App 조회로 Cycle Report `cycle:C2`와 Analysis `step:C2/S4`가 `references` 1개(원래 Verify step:C2/S3)를
  돌려주는 것을 확인했다. Cycle Report 선택 화면 캡처는 이 mock Host에서 클릭이 Step 노드에 가려져 남기지 못했다.

이 mock Host는 MCP App 번들(`monitor.html`)이 아니라 번들의 원본 모듈을 같은 코드로 렌더링한 것이다. 세로
그래프, Host 테마 없음. **Codex·Claude Desktop(Claude Code 탭)의 실제 화면은 사용자 확인 대기**다.

## 4. 남은 사용자 확인 항목

1. 이 브랜치의 설치본(또는 개발 빌드 직접 등록)으로 Codex와 Claude Desktop에서 새 빈 폴더 → 작은 데이터 분석
   요청(그림을 요구하지 않음) → Verify 노드에 플롯이 첨부되는지, 노드 배지·상세 패널·Cycle Report 인용이
   화면에 보이는지. 홍보용 캡처는 그 화면으로 남긴다.
2. `GIL_EVAL_MODEL`을 정한 뒤 `node mcp-app/guidance-eval.mjs`와 `node mcp-app/interview-workflow-eval.mjs`
   (유료) 실행과 `visualAttachedUnprompted` 판정 확인.
3. data-science·frontend 시나리오(`scenarios/wave1`)에서 `visual_evidence_attached` 채점.
