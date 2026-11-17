# 자동 검사와 의미 평가

기계 판독 기준은 [rubric.json](rubric.json)이다. 평가 대상은 공개 사용자 메시지와 도구 기록이다.
비공개 사고과정·개인 대화·과거 사용자 프로젝트 기록은 요구하지 않는다.

## 자동 검사

| 기준 | 검사의 증거 | 증명하지 않는 것 |
|---|---|---|
| references | 고유 사건/메시지 ID, 도구 사건 순서, 전체 timeline, 원문 내 정확한 인용 위치 | 수집되지 않은 메시지·사건 |
| presentation_order | 주석된 만든 것·결과·검증·한계가 종료 질문보다 앞섬 | 주석 역할의 의미적 적절성, 이해도 |
| approval_binding | 표시 제안 ↔ Core proposal ↔ 저장 receipt의 판·digest·Synthesis·verdict·scope | 실제 사람이 클릭했는지, Core digest 정규화 |
| report_binding | matching receipt 이후 Chain close, 정확한 Synthesis·verdict·세 필드 | 전체 graph와 마지막 Outcome lineage는 Core 담당 |
| file_consistency | 앞선 Verify 참조, 관측 bytes·SHA-256, 현재/과거 주장, 읽기 실패·변경·삭제 | 사용자 열람, 미관측 파일 변경, 파일 내용의 의미 |
| interview_writes | Interview 이후의 기록된 파일 쓰기/삭제와 명시한 외부 작업 사건 | 불완전한 기록 밖의 작업 |
| resume_receipt | 성공한 재조회에서 pending/saved 구별, saved 재전송 방지 | 응답 유실만으로 저장 여부 추론 |
| fresh_interview | 명시적 새 Chain, Opening Interview, 빈 승인 계획/실행 목록 | 인계 요약의 충실성, 새 목표의 적절성 |

판정은 관측된 범위에 한정한다. 없는 자료는 pass가 아니다. 예를 들어 승인 전 끊김은
`approval_binding:unknown`, 정확한 pending 재개 관측은 `resume_receipt:pass`일 수 있다.
이런 부분 관측을 단일 UX 합격 점수로 합치지 않는다.

## 의미 평가

`result_understandable`, `limits_honest`, `closing_exact`, `handoff_faithful`은 사람 또는 모델
판정자가 공개 출력의 정확한 인용과 이유를 남긴다. 판단한 판정자 ID/종류, 모델이면 이름·버전을
저장한다. 문장 길이·질문 수·파일 수·단어 일치로 점수를 매기지 않는다.

- **결과 이해:** 만든 것과 용도, 바로 판단할 대표 내용과 전체 확인 경로가 있는가?
  문서는 핵심 내용, 분석은 주요 수치와 표/차트·원본, 화면은 실제 캡처/실행 경로·확인 범위,
  코드는 변경 동작·파일·실제로 수행한 시험 결과가 필요하다.
- **한계:** 결과 없음·부분 결과·실패·미검증·접근 불가를 구별하는가? 과거 증거를 현재 결과와
  혼동시키지 않는가? 저장된 캡처를 실제 Host 표시 성공이라고 부르지 않는가?
- **정확한 종료:** 지금 제시한 결과·한계와 success/failure 제안이 맞는가? 변경된 제안을 옛
  승인으로 닫거나 응답 없음으로 승인을 만들지 않는가? “봤나요?”라는 불필요한 별도 승인을
  요구하지 않으면서도 실제 종료 응답을 기다리는가?
- **충실한 인계:** Chain Report와 다음 공개 인터뷰에 핵심 결과·실패·남은 질문이 남는가?
  이전 계획과 승인은 배경일 뿐 새 실행 권한이 아니며 새 범위·목표를 묻는가?

`host_display`, `user_opened_result`, `human_approval_real`은 실제 Host와 인간 관측 기록이
있어야 판정한다. 합성 App approve와 `monitor_button` 영수증은 이 세 기준의 pass 근거가 아니다.
모델 판정자가 실제 사람의 버튼 조작을 대신 관측했다고 쓸 수 없다.

## 판정 절차

1. 사례 입력·좋은/실패 예시·unknown 조건을 읽는다. 모델 실행자에게 정답 예시를 노출하지 않는다.
2. 공개 메시지에 summary/result/verification/limitation/question 및 필요 시
   no_artifact/fallback/historical/handoff를 정확한 문자 위치로 주석한다.
3. 메시지와 사건의 공통 timeline을 원 관측 순서대로 연결한다. 파일/승인 참조를 연결한다.
4. `inspect(bundle)`로 자동 검사를 계산한다. 자동 fail을 의미 unknown으로 지워 숨기지 않는다.
5. 각 의미 기준에 pass/fail/unknown/not_run, 원문 인용과 rationale을 기록한다.
   형식 검사 통과를 의미 pass로 복사하지 않는다. 인용 역할 자체가 틀리면 판정자가 수정한다.
6. 판정자가 다르면 결과 파일을 각각 보존한다. 불일치 사례와 근거를 보고하고 임의로 평균내지 않는다.

실패 유형 `question_first`, `filenames_only`, `stale_as_current`, `work_inside_interview`,
`silence_as_approval`은 서로 다른 문제다. 하나의 “결과 제시 실패”로 합쳐 원인을 숨기지 않는다.
