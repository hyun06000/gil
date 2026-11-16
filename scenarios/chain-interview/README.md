# Chain Interview 평가 묶음 — 심층 인터뷰·간결한 제안·최소 문제 Define

GIL Chain의 Opening Interview가 **필요한 만큼 깊게 묻고**, Synthesis 제안이 **짧지만 제약을 잃지 않고**,
Experiment의 Define이 **승인 항목 하나와 같은 범위**인지 평가하기 위한 **합성 사례와 오프라인 검증 묶음**이다.
제품 코드와 독립적이며, 이 묶음 자체는 모델을 호출하지 않고 제품 동작도 검증하지 않는다.
실행 도구는 [`tools/chain-interview-eval/`](../../tools/chain-interview-eval/README.md)에 있다.

평가 대상은 **사용자에게 보이는 질문·제안·완료 메시지와 도구 기록**이다. 모델의 비공개 사고과정은 요구하지도 평가하지도 않는다.

## 사례 목록

| id | 범주 | 평가 단계 | 요약 |
|---|---|---|---|
| [CI-01](cases/CI-01.json) | ambiguous_request | interview, proposal | “매출 자료를 잘 분석해줘”. 목적·대상·형태를 물어야 하고 도구 선택은 묻지 않는다 |
| [CI-02](cases/CI-02.json) | multi_goal | interview, proposal | 점검·분석·차트·제안서가 한 요청에. 우선순위를 묻고 독립 검증 가능한 항목으로 나눈다 |
| [CI-03](cases/CI-03.json) | conflicting_constraints | interview, proposal | “고객 10명 이름”과 “개인정보 없음”이 충돌. 사용자 결정으로 올려야 한다 |
| [CI-04](cases/CI-04.json) | missing_inputs | interview, proposal | 없는 2분기 파일과 접근 불가 회계 시스템. 추정·자격 증명 요청으로 메우지 않는다 |
| [CI-05](cases/CI-05.json) | already_specific | interview, proposal | 이미 구체적인 요청. 답해진 것을 다시 묻지 않고 범위를 더하지 않는다 |
| [CI-06](cases/CI-06.json) | proposal_omits_constraints | proposal | 짧은 제안이 외부 공유 금지·9월 신뢰 불가·제안서 비범위를 지웠는가 |
| [CI-07](cases/CI-07.json) | over_fragmented_items | proposal | ‘파일 열기’·‘열 이름 확인’처럼 독립 결과 가치가 없는 아홉 항목 |
| [CI-08](cases/CI-08.json) | partial_define_claimed_complete | define | 승인 항목 B의 일부만 Define하고 “B 완료”로 제시 |

모든 사례는 같은 합성 배경(합성 문구점의 2026년 3분기 판매 CSV)을 쓴다. 실제 사용자·회사·자료가 아니다.
[`comparisons/sales-report-bundled-vs-separable.json`](comparisons/sales-report-bundled-vs-separable.json)은 같은 큰 목표에 대해
**모든 일을 한 실험에 묶은 안**과 **결과를 독립적으로 검증할 수 있게 나눈 안**을 나란히 두고, 왜 후자가 기준인지와
의미가 같은 다른 분할을 적어 둔다. CI-02·CI-07·CI-08이 이 파일을 참조한다.

## 사례 파일의 구성

```text
cases/CI-NN.json
├─ input                 사용자 요청, 합성 자료 설명(exists로 부재 표시), 선행 상태(승인 판·닫힌 실험), 합성 사용자 답변
├─ clarifications        필요한 추가 확인 — required / optional / must_not_ask 와 이유. required마다 합성 답변이 있다
├─ good_examples         좋은 질문·제안·Define·메시지. meets가 가리키는 의미 기준, note, acceptable_variants
├─ bad_examples          실패 예시. failure_modes(의미)와 declared_structural_violations(구조 검사가 잡아야 하는 것)
├─ judgement             automatic(구조 검사 id), semantic(의미 기준 id), basis, not_yet_judgeable(아직 판단할 수 없는 것)
├─ comparison_ref        비교 파일 (있으면)
└─ spec_refs             근거 명세
```

- 질문 예시의 보기는 `role`(option / unsure / free_text)을 명시한다. Cycle Model §14의 두 탈출 경로(“아직 잘 모르겠음 — 더 작은 질문 필요”, “직접 입력”)는
  **구조**로 검사하고 문구는 검사하지 않는다.
- 제안 예시는 `gil_plan_propose`의 `draft`와 같은 모양(scope 다섯 필드, items, synthesis)이다. Define 예시는 `bound_item`·`problem`·`success_condition`이다.
- **좋은 표현을 한 문장으로 고정하지 않는다.** 각 좋은 예시의 `acceptable_variants`가 의미가 같은 다른 형태를 적고, 어떤 검사도 예시 문장과의 일치를 보지 않는다.
- 실패 예시 가운데 구조 검사가 잡는 것(`declared_structural_violations`)과 **구조는 완전하지만 의미가 틀린 것**(빈 배열)을 구분한다.
  후자가 많다. 간결함이 제약을 지운 제안(CI-06), 조용히 한 제약을 버린 제안(CI-03), 아홉 조각 제안(CI-07)은 모두 구조 검사를 통과한다.
- fixture 안의 승인(CI-08의 승인 판)은 **합성**이다. 실제 사람의 Monitor 버튼이 아니며 `approval.kind: synthetic`으로 표시한다.

## 판정 어휘

`pass` · `fail` · `unknown`(관측이 부족해 판단 불가) · `not_run`(실행하지 않음). 실행하지 않은 사례는 not_run, 기록이 끊긴 사례는 unknown으로 남긴다.
둘 다 pass가 아니며, 검사기는 이 상태를 올려 주지 않는다. 자세한 기준은 [rubric.md](rubric.md), 결과 파일 형식은 [results/FORMAT.md](results/FORMAT.md),
나중에 실제 모델로 반복하는 절차는 [PROTOCOL.md](PROTOCOL.md)를 본다.

## 기준 계약과 커밋

사례는 검토 기준 커밋 `7a76f4e`(브랜치 `codex/chain-first-roadmap-20261117`, 2026-11-17 “Specify Chain-first priorities and smallest actionable experiments”)가
추가한 요구를 대상으로 한다: Cycle Model §5.1 심층 인터뷰, §7.1 지금 실행 가능한 가장 작은 문제, §16 간결하고 명징한 제안,
Approved Plan의 “실행 항목의 크기와 Define의 일치”, Roadmap §2의 2026-11-17 현재 구현 순서(Chain 최우선, 네 가지 요구).
`spec_refs`의 해당 절 주소는 그 커밋 기준이다. 이 묶음을 만든 작업 가지는 main `2b24746`에서 갈라졌고 `7a76f4e`는 그 후손이므로,
main에만 있는 상태에서는 파일은 존재하지만 그 절 제목이 아직 없다. 문서 연결 검사는 파일 존재만 본다.

## 하지 않는 주장

- 이 묶음은 제품 코드·UI·공통 지침을 바꾸지 않았고 제품 행동을 검증하지 않았다. 사례가 기준 계약(Cycle Model §5·§7·§14–16, Approved Plan)과 맞는지만 검토했다.
- 모델을 실행하지 않았다. 예시 결과 파일은 형식을 보여 주는 합성 예시이며 어떤 모델의 성능도 나타내지 않는다.
- 사용자 필드 테스트, 실제 사용자 기록, 외부 데이터, credential을 쓰지 않았다.
- 로드맵 §2 구현 순서 1번(안내·간결화)·2번(설명서·안내 보완)이 통합된 뒤 이 묶음으로 전후를 비교할 수 있도록 지침 버전(workflow.md 해시·commit)을 결과에 남기게 했지만, 그 통합을 기다리거나 완료로 세지 않는다.
