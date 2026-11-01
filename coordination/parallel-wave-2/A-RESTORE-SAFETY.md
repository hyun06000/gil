# A — 복원 미리보기 안전 판정 수정

Common instructions: `coordination/parallel-wave-2/README.md` and `coordination/parallel-wave-2/INTERFACES.md`.

예상 branch: `claude/wave2-restore-safety`.
전용 worktree에서 README.md와 INTERFACES.md의 공통 규칙을 적용하고 분석만 하지 말고 수정·자동시험·로컬 커밋까지 완료한다.

먼저 `../parallel-wave-1/review/REVIEW.md`의 R1/R2와 regressions.rs를 읽고 두 실패를 재현한다.
Artifact/Storage 명세, src/restore/preview.rs와 tests.rs의 Probe/Readiness 계약을 확인한다.

소유 파일:
- src/restore/preview.rs, src/restore/preview/tests.rs 및 이 모듈의 신규 전용 하위 파일.
- spec/proposals/M3_Restore_Preview_v0.1.md의 이번 수정 설명.
- 필요한 최소 의존성에 한해 Cargo.toml/Cargo.lock (A 단독). feature는 바꾸지 않는다.
- coordination/parallel-wave-2/results/A-RESULT.md.

작업:
1. root kind 조회를 무시하는 경로를 고친다. 실제 디렉터리임을 확인하지 못한 root는 Candidate가
   될 수 없다. Symlink/Missing/File/Other/Io/Unsupported, 빈 Manifest일 때도 전체 판정에 반영한다.
   디스크 조회 전에 거절할 수 있는 경로는 안전하게 거절하고 링크를 따라 외부를 조회하지 않는다.
2. literal name 부재를 즉시 Missing으로 대체하는 동작을 고친다. 실제 Mac에서 NFD 파일을 만들고
   NFC target이 같은 파일에 닿는 재현을 정식 회귀시험에 넣는다. 대소문자뿐 아니라 정규화 별칭을
   검출하며, 검증하지 못한 동등성은 미확인으로 거절한다. 기존 대소문자 충돌 회귀를 보존한다.
3. root/ancestor/leaf 검사와 읽기 시점 한계를 설명한다. 이것을 Windows handle/TOCTOU 안전성이나
   apply 허용으로 주장하지 않는다. 제품 restore/revisit 차단과 실제 apply 코드에는 손대지 않는다.
4. 기존 미추적 파일의 내용이 보존되고 preview 전후 파일/기록 bytes가 같음을 검사한다.
   Unicode 지원을 추가할 때 일부 표만 구현해 전체 정규화 지원이라고 부르지 않는다.

필수 검증: 새 두 재현과 root 오류/빈 입력 시험, preview 전체, 기본 Core 시험/빌드,
approved-plan,visual-authoring,product-safety 프로필의 preview 시험/빌드. Rust는 Wave 2 큐 사용.
의존성을 바꿨으면 라이선스/고지 변경을 보고하고 고지 hash는 통합 담당자에게 남긴다.
합격은 R1/R2가 정식 시험으로 통과하고 기존 보존·거절 계약이 유지되는 것이다.
