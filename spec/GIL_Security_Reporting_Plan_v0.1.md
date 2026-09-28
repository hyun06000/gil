# GIL Security Reporting Plan v0.1

상태: 준비 · 비공개 보안 제보 창구 미개통 (2026-09-28).
이 문서는 운영 계획이며 활성화된 SECURITY 정책이 아니다.

## 현재 경계

`hyun06000/gil`은 비공개다. 보안 연락용 이메일은 정하지 않았고 응답 기한도 약속하지 않는다.
취약점 세부사항, 악용 방법, 비밀 값이나 실제 사용자 자료를 일반 issue/PR에 게시하지 않는다.
비공개 저장소의 현재 issue도 나중에 공개될 수 있으므로 보안 제보함으로 취급하지 않는다.
유지관리자 프로필과 GitHub abuse 신고 창구 역시 GIL의 취약점 제보함이 아니다.

GitHub Private Vulnerability Reporting은 공식 문서상 **공개 저장소**에서 소유자/관리자가
활성화하는 기능이다. SECURITY.md를 작성하는 것만으로 켜지지 않는다.
2026-09-28 현재 저장소의 해당 REST 조회는 404였으며, 제보 가능 상태를 확인하지 못했다.
이를 켜려고 저장소를 공개하거나 비공개 설정을 우회하지 않는다.

## 결정된 경로

2026-09-28 유지관리자는 **추후 공개 전환 시 GitHub 비공개 취약점 제보를 사용**하기로 결정했다.
이후 배포 준비 완료 뒤 공개·배포하는 순서를 승인했지만 게이트는 미완료다. 지금 PVR 설정을
바꾸거나 SECURITY.md를 활성 정책으로 게시하지 않는다. 수신 담당자는 저장소 소유자이고
공개 전환 절차에서 권한·알림을 검수한다.
그 전에 다른 제보 경로가 필요하면 실제 연락처와 공개 범위를 따로 결정한다.
주소를 추정하거나 개발자의 개인 이메일을 공개하지 않는다.

## 활성화 체크리스트

- [x] 유지관리자가 접수 방법과 담당자를 확정한다 — 공개 시 GitHub PVR, 저장소 소유자.
- [ ] 설정 변경을 별도 승인받고 실제 제보 진입점·수신 권한·알림을 확인한다.
  GitHub 경로라면 공개 전환 승인과 PVR 활성화를 한 검수 절차로 묶으며, 확인 전 공개 준비를 완료로 표시하지 않는다.
- [ ] 공개 범위, 지원 대상 버전, 제출 가능한 최소 재현 정보와 비밀 제외 기준을 SECURITY.md에 쓴다.
  미출시 preview의 한계를 적고 과거 모든 버전 지원·수정 기한·보상을 약속하지 않는다.
- [ ] 실제 접수 링크를 SUPPORT·CONTRIBUTING·issue 선택 화면에 연결하고 동작을 확인한다.
- [ ] 공개 준비 장부의 보안 항목을 증거와 함께 갱신한다.

테스트 목적의 가짜 취약점 보고나 공개 issue를 만들지 않는다. 계획 수립, 설정 활성화와
실제 신고 접수 검수는 서로 다른 단계다. 준비 중인 동안 SECURITY.md를 만들어 운영 중인
비공개 제보 기능이 있다고 표시하지 않는다.

## 근거와 관련 문서

- [GitHub: repository의 private vulnerability reporting 설정](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/configure-vulnerability-reporting/configure-for-a-repository)
- [GitHub: 비공개 취약점 제보 방법](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/report-privately)
- [공개 준비 게이트](GIL_Open_Source_Readiness_v0.1.md)
- [도움받기](../SUPPORT.md)
