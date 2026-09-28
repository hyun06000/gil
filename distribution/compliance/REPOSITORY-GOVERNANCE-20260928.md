# Repository governance checkpoint — 2026-09-28

## 결정과 실제 설정

- 새 `hyun06000/gil`은 계속 비공개다. 공개 전환·정식 release·marketplace 게시를 승인하지 않았다.
- 유지관리자 승인으로 기존 개발 저장소도 비공개로 전환했고 REST 응답으로 확인했다.
  기존 Git 이력·로컬 변경·설치본은 삭제하거나 다시 쓰지 않았다.
- 이전에 만들어진 공개 fork 1개는 비공개 전환으로 회수되지 않는다. 기존 clone·외부 사본도 같다.
  기존 Pages에 별도 custom domain은 없었다. 이 조치는 과거 노출 정보를 완전히 제거했다는 뜻이 아니다.
- 지금부터 `main` 변경은 작업 브랜치 → PR → 검토·시험 → 병합으로만 진행한다.
  직접 push·강제 push·PR 규칙 우회를 하지 않는다. GitHub에서는 merge request를 pull request(PR)라고 부른다.
- 단독 유지관리자가 자기 PR을 병합할 수 있게 타인 승인 수를 필수로 요구하지는 않는다.
  이는 PR 생략 허용이 아니며, 공개 리뷰와 CI 성공은 별도 증거다.

## 서버 강제 보호 — 미완료

비공개 `gil`의 rulesets 및 classic branch protection 조회는 모두 **HTTP 403**으로 거절됐다.
응답은 GitHub Pro로 업그레이드하거나 공개 저장소로 전환해야 한다고 명시했다. 기존 개발 저장소도
비공개 전환 뒤 branch protection 조회가 같은 이유로 거절됐다. 결제·요금제 변경은 하지 않는다.
정보 보호를 위해 비공개로 정한 저장소를 보호 규칙 사용 목적으로 다시 공개하지 않는다.

기존 저장소의 공개 상태에는 main/tag 규칙이 있었지만, main 규칙은 삭제와 force push만 막았고
PR을 요구하지 않았다. 전환 전에 두 규칙의 원본을 저장소 밖 비공개 감사 자료로 보존했다.
기존 규칙의 존재를 비공개 전환 뒤 유효한 PR 강제 보호라고 주장하지 않는다.

운영 규칙과 개발 지침은 적용했지만, **현재 main 직접 push의 서버 차단은 보장되지 않는다.**
GitHub Pro 등 지원 요금제가 준비되면 다음을 적용·재조회한다.

- [ ] 두 저장소의 `refs/heads/main`에 활성 규칙 적용.
- [ ] PR 필수, branch 삭제·force push 금지.
- [ ] 관리자·앱·사용자의 bypass 목록 없이 적용.
- [ ] 원래의 다른 보호 규칙은 보존하고 실제 유효 규칙을 재조회.
- [ ] 승인된 CI를 같은 PR head에서 검증하고 PR을 통해 병합.

## 보안 제보

추후 공개 전환 시 GitHub Private Vulnerability Reporting을 활성화하고 수신을 확인하기로 결정했다.
현재는 창구 미개통 상태이며 [별도 계획](../../spec/GIL_Security_Reporting_Plan_v0.1.md)을 따른다.

## 근거

- [GitHub: protected branches의 요금제와 관리자 우회 기본값](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)
- [GitHub: visibility 변경의 영향](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility)
- [공개 준비 장부](../../spec/GIL_Open_Source_Readiness_v0.1.md)
