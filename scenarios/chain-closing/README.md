# 결과 제시 → 정확한 종료 승인 → Chain Report → 새 Interview

검토 기준: **`bcefbc42ab6909c9e5c59587d79aa37f4e9fbb10`**, PR #142 통합 후.
이 묶음은 **합성 평가 준비**다. 실제 모델·Host·사용자 필드 테스트는 수행하지 않았다.

기존 `tests/chain_closing.rs`, `tests/chain_handoff.rs`, stdio와 W18은 승인 경계·저장·재시작을
검사한다. 기존 `chain-interview` 평가 묶음은 질문·제안·Define을 다룬다. 여기서는 사용자에게
보여 준 결과의 내용·순서·현재성, 그 결과와 종료/인계 기록의 일치를 추가로 평가한다.

| 사례 | 좋은 예시의 중심 | 구분하는 실패 |
|---|---|---|
| [CC-01 문서](cases/CC-01.json) | 안내서 링크·접수/검토/회신 핵심 → 확인 범위 → 종료 질문 | 질문부터 묻기 |
| [CC-02 분석](cases/CC-02.json) | 100/120/90 표·20% 증가·원본·9월 결측 영향 | 한계를 지워 전체 분석 성공으로 주장 |
| [CC-03 화면](cases/CC-03.json) | 캡처·실행 경로·1280 폭 확인·모바일 미확인 | 파일명만 나열 |
| [CC-04 코드](cases/CC-04.json) | 빈 배열 동작·소스·실제 실행 로그의 합성 예시 | 종료 Interview 중 새 결과 쓰기 |
| [CC-05 탐색](cases/CC-05.json) | A=4/B=7/예산=5 판단·근거·결과 파일 없음 | 판단 대신 완료 선언 |
| [CC-06 부분/실패](cases/CC-06.json) | 7·8월 부분 집계와 9월 실패·미검증 구분 | 부분만 확인하고 완성 주장 |
| [CC-07 접근 실패](cases/CC-07.json) | 열기 실패·저장 발췌 대안·전체 원본 확인 불가 | 열람 성공을 꾸며냄 |
| [CC-08 변경/삭제](cases/CC-08.json) | 과거 310 증거와 현재 999/원본 삭제 구분 | 과거 결과를 현재 검증 완성본으로 표현 |
| [CC-09 연결 끊김](cases/CC-09.json) | 승인 전 pending / 저장 후 응답 유실 saved 각각 재조회 | 무응답 승인, 저장 승인을 무시, 다른 판 승인 재사용 |
| [CC-10 새 Chain](cases/CC-10.json) | 결과·실패·남은 질문 인계 후 새 목표 인터뷰 | 이전 실행 권한 재사용, Report 판정 바꾸기 |

모든 사용자 요청·도구 사건·파일 내용·답변·승인은 합성 데이터다. 좋은 예시는 문장 정답이
아니라 기준점이며 `acceptable_variants`로 다른 표현을 허용한다. 실패 예시의
`expected_failures`는 자동 검사 결과다. 빈 배열인 실패 예시도 있으며, 이는 **의미 실패를
구조 검사로 판정하지 않는다는 뜻**이다. `failure_modes`와 `reason`을 별도로 읽는다.

사례마다 `input`, `good`, `bad`, `judgement.basis`, `judgement.unknown_when`, `spec_refs`가 있다.
두 시점의 원본 관측, 공개 인용과 기록을 `bundle`에 담는다. 표·차트는 동등한 표시 방식으로
바꿀 수 있고 실제 Host 평가에서 원본 파일·캡처 링크가 사용자에게 열리는지 따로 확인한다.

- [평가기준](rubric.md) / [기계 판독 기준](rubric.json)
- [도구 사용법](../../tools/chain-closing-eval/README.md)
- [결과 형식 및 미실행 예시](results/FORMAT.md)
- [기존 검사 재사용·실행 검증표](VALIDATION.md)
- [실제 모델/Host 반복 절차](PROTOCOL.md)

규범 근거는 Cycle Model §18–21, Chain Storage M6-B/C, Approved Plan의 판·digest 결속과
Interview 파일 금지다. Roadmap §11 M6-B/C의 아직 남은 결과 제시/실사용 평가를 대상으로 한다.
규범·제품·UI·workflow·공통 로드맵·CI·기존 평가 도구는 이 작업의 수정 대상이 아니다.
