# 시각 보고서 format 4 → 5 이관 절차

2026-10-13. 개발 검수 절차. 실제 사용자 프로젝트의 이관 실행·설치 교체 승인이 아니다.
기준: 시각 보고서 저장 계약 §5, 작성 계약 G5-D. 기존 format 4 읽기는 자동 이관하지 않는다.

**2026-10-14 제품 절차 확정:** §1–4는 개발 도구와 실행 장부다. 일반 사용자용 신규
생성·승인 이관의 구현 기준은 **§5**를 따른다. 명세 확정이며 구현·배포·실제 이관 완료가 아니다.

## 사용자에게 설명할 내용

> 이미지가 있는 보고서를 쓰려면 기록 형식을 한 번 바꿔야 합니다. 이전 보고서와 작업
> 이력은 그대로 남습니다. 먼저 원래 기록과 필요한 파일을 별도로 백업하고, 복제본에서
> 새 형식이 정상인지 검사하겠습니다. 바뀐 기록은 옛 프로그램에서 열거나 쓸 수 없습니다.
> 복구가 필요하면 이후에 추가된 기록도 보존한 뒤 선택하겠습니다. 지금 원본에 적용할지는
> 검수 결과를 본 다음 따로 결정해 주세요.

## 1. 중지 조건과 사전 확인

- 정확한 프로젝트와 대상 형식을 확인한다. 부모 프로젝트 추측·전체 홈 디렉터리 사용 금지.
- 모든 Agent 쓰기를 멈추고 같은 프로젝트의 잠금을 얻는다. 잠금 파일을 지우지 않는다.
  외부 편집기·동기화 프로그램은 advisory lock을 따르지 않으므로 따로 정지해야 한다.
- 읽기 전용 Session으로 판독한다. 복원 transaction이나 정리해야 할 흔적이 있으면 이관을
  멈춘다. 이관을 명분으로 복구·cleanup·파일 삭제를 먼저 실행하지 않는다.
- format 4만 받는다. 이미 5이면 경계를 재작성하지 않고, 더 오래되거나 알 수 없는 형식,
  손상·누락 객체·링크/특수 항목·권한 문제는 별도 조사한다. 링크를 몰래 따라가거나 빼지 않는다.
- 열린 Cycle은 버리거나 임의로 닫지 않는다. 열린 Verify도 그대로 이관할 수 있으나,
  이후 close에는 시각 선언이 필요하다는 차이를 알린다. 과거 닫힌 Verify는 미기재로 남는다.
- 공간·권한과 호환 reader/writer를 확인한다. Windows 원본 이관은 native 안전성 검수 전 금지.

## 2. 백업과 복제본 검수

1. 프로젝트 밖의 새 비공개 디렉터리에 백업한다. 기존 백업을 덮어쓰거나 hardlink로
   공유하지 않는다. 디렉터리 0700, 파일 0600을 기본으로 하며 기록 노출을 확대하지 않는다.
2. `.gil` 전체(state·모든 Artifact 객체·미참조 항목 포함)를 보존한다. state만 복사한 것은
   완전한 기록 백업이 아니다. 현재 작업 파일과 범위 설정도 별도 보존한다. Snapshot이
   제외 파일·미확정 변경을 모두 담았다고 가정하지 않는다. Git은 이 백업의 대체물이 아니다.
3. 파일 목록·종류·길이·digest와 원래 권한을 백업 장부에 남기고 읽어 대조한다.
   원본과 백업 각각에서 `ReadOnlySession::verify_snapshot_objects`로 모든 Snapshot의
   manifest와 참조 blob bytes를 검증한다. 일반 state 판독 성공만으로 객체의 완전성을
   판단하지 않는다. 이 검사는 작업 파일/미참조 객체/독립 시각 자료의 전체 백업 검증을
   대신하지 않으므로 파일 목록 대조도 함께 유지한다.
   복사 전후 원본 목록·내용이 달라졌으면 후보를 적용하지 않는다. 백업 파일과 디렉터리를
   sync하고, 실패하면 검증된 백업이라고 표시하지 않는다.
4. 백업은 그대로 두고 별도의 새 작업 복제본을 만든다. 그 복제본의 잠금을 얻은 상태에서
   `prepare_visual_upgrade` 후보를 저장·재판독한다. 이것은 일반 사용자용 명령이 아니다.
5. state의 `format` 변경과 빈 `visuals`/이관 경계 추가만 허용한다. 이를 제외한 YAML 값 전체를
   비교해 Report 문자열(개행·공백·Unicode), Graph, Cycle, Journey, Existence, Will,
   Snapshot 참조와 pending을 보존했는지 확인한다. 파일 직렬화가 달라질 수 있으므로 원본
   state bytes는 백업에서 별도로 정확히 보존한다. 객체 bytes와 작업 파일은 변경하지 않는다.
6. 백업으로 세 번째 독립 복구본을 만들어 실제 reader로 연다. 복원본의 모든 파일 bytes가
   원본과 같고 모든 참조를 읽을 수 있어야 한다. 원본 위치에 덮어쓰는 복구 시험은 금지한다.
7. 실제 구형 배포 바이너리가 새 복제본의 쓰기를 거절하고 기록을 바꾸지 않는지 확인한다.
   현재 reader에서 format 숫자만 바꿔 본 시험으로 이 항목을 대신하지 않는다.

## 3. 원본 적용 게이트 — 개발 도구, 합성 프로젝트에서만 검수

`examples/visual_migrate.rs` / `src/visual_migration.rs`에 Unix 개발용 제자리 적용 도구를
추가했다. 설치된 CLI/MCP/Plugin의 명령이 아니며 자동 이관을 추가하지 않는다. 실제 사용자
프로젝트 적용·설치 후보 활성화는 별도 게이트다. Windows에서는 실행을 거절한다.

독립 단계는 다음과 같다. 아래 경로는 설명용이며 실제 사용자 폴더에 그대로 적용하지 않는다.

```sh
cargo run --locked -p gil --example visual_migrate -- prepare /absolute/synthetic-project /absolute/new-backup
cargo run --locked -p gil --example visual_migrate -- status /absolute/synthetic-project /absolute/new-backup PLAN_SHA256
# 검수 결과 확인과 명시적 승인 이후에만 실행하는 별도 단계:
cargo run --locked -p gil --example visual_migrate -- apply /absolute/synthetic-project /absolute/new-backup PLAN_SHA256
```

- `prepare`: 원본의 기존 잠금을 얻어 읽기 전용 검사 → 전체 목록/bytes 수집 → 별도 새
  비공개 백업(`original`) → 새 형식 복제본(`candidate`) → 백업에서 읽어 만든 독립 복구본
  (`restored`) → 세 복제본의 bytes·Snapshot 전수 검사 → 원본 재대조 → `plan.json`과 sync.
  준비 결과는 plan의 SHA-256이다. 적용에는 이 값을 명시적으로 다시 전달해야 한다.
- plan은 원본 디렉터리 identity, 원래 파일/빈 디렉터리 목록·권한·길이·SHA-256, 이전/새
  state digest를 기록한다. 원문 bytes는 `original`에 있다. ACL/xattr·소유권·시간 정보까지
  복원하는 범용 백업은 아니다. 복사본은 0700/0600이며 원래 실행 권한은 장부에만 보존한다.
- `apply`: 다시 잠금 → plan 해시·원본 identity·원본 전체·세 복제본 재검증 및 sync → 같은
  serializer로 후보를 재계산 → `.gil/state.yaml.visual-migration` 배타 생성 → bytes 쓰기/
  sync/재판독 → 원본·잠금·임시 파일의 identity와 내용 최종 확인 → 유지한 디렉터리 fd에
  상대적인 `renameat` 한 번 → `.gil` sync → 별도 `applied.json` receipt와 백업 디렉터리 sync.
  교체되는 원본 파일은 state 하나뿐이다. 기존 Report·객체·작업 파일은 덮어쓰지 않는다.
- 실패 시 부분 백업과 임시 파일도 자동 삭제하지 않는다. 기존 백업·임시 파일을 덮어쓰지
  않는다. rename 이후 오류는 “적용되지 않음”으로 단정하지 않는다. receipt 유무만으로
  적용 여부를 판정하지 않는다.
- `status`: 잠금 아래 읽기 전용 객체 검사와 state digest 비교. `not_applied`는 옛 state,
  `applied_state_present`는 준비한 새 state, `changed_or_unknown_preserve_all`은 그 외다.
  마지막 값에는 이관 후 정상적으로 추가된 보고서도 포함된다. 첫 두 값도 작업 파일 전체의
  불변이나 물리적 전원 차단 내구성을 뜻하지 않는다. 판독 자체가 실패하면 보존하고 멈춘다.
- 도구는 원본/백업 경로를 추측하지 않는다. 현재 개발 한도는 전체 파일 bytes 256 MiB,
  20,000 항목, 깊이 64이며 UTF-8 이름만 처리한다. 링크·hardlink·특수 파일, 남은 state
  임시 파일, 누락된 기존 project.lock, 지원 범위를 벗어난 입력은 조용히 제외하지 않고
  거절한다. 사전 용량 추정은 공간 확보 보장이 아니므로 각 write/sync 실패를 처리한다.

외부 편집기·동기화 등은 반드시 멈춘 상태여야 한다. retained fd와 마지막 재검사도 같은
사용자 권한의 악의적 프로세스가 최종 검사 직후 쓰는 모든 경합을 제거하지 못한다. macOS의
`sync_all`은 기존 저장 계약과 같은 파일 시스템 계층 보장이다. 실제 저장 장치의 전원 차단
복구, Windows native 동등 구현, 큰 프로젝트·메타데이터 완전 복원은 아직 통과가 아니다.

적용 결과가 불확실하면 이전/새 state digest와 백업을 읽어 판정한다. 자동 재시도·다운그레이드·
백업 덮어쓰기는 금지한다. 이관 후 새 Report나 이미지가 추가됐다면 옛 백업을 복원하는 행위는
그 새 기록을 버릴 수 있다. 먼저 현재 전체 기록도 보존하고, 복구는 별도 복제본에서 검증한 뒤
사용자에게 차이를 설명하고 선택을 받는다. 원본과 백업 모두 보존하는 것이 실패 기본값이다.

## 4. 검수 범위

`tests/visual_migration.rs`는 생성한 합성 fixture만 사용하는 **절차 리허설**이다.
범용 백업/이관 프로그램이 아니며 symlink race·ACL·xattr·대규모 파일·실제 전원 중단에 대한
제품 보장을 제공하지 않는다. 합성 데이터에 한정해 전체 byte 목록 대조·독립 복구·후보 저장
실패·원본 보존·구형 writer 거절을 검사한다. 실행 결과는 아래에 별도로 기록한다.

### 실행 결과 (2026-10-13, macOS)

- 열린 Verify와 닫힌 Verify를 각각 백업→새 복제본 이관→제3 복구본으로 복원했다.
  기존 state 값 전체(새 format/visuals 제외), 작업 파일과 객체 bytes가 일치했다.
- 별도 닫힌 Cycle fixture에서 Cycle Report·Journey 보존과 과거 Verify에 시각 선언을
  소급 생성하지 않는 것을 확인했다. 열린 Verify는 새 선언 없는 close를 거절했다.
- 후보 state 임시 파일 충돌 시 원본·백업·후보의 기존 state를 보존했다. 이미 format 5인
  후보는 재이관을 거절했다. busy/active restore 시 사전 판독이 거절되고 흔적이 남았다.
- blob 누락, bytes 손상, 외부 파일 symlink를 대상으로 전수 검사 거절·비수정 확인.
- 실제 설치된 macOS arm64 preview.3 바이너리의 `close`는 format 5를 거절했고 후보의
  전체 파일 bytes가 불변이었다. 바이너리 SHA-256:
  `3572786190ec4a8ad3b703ebe5c8ffd9b170a4e6c7da1ea2776abfc12c520a47`.
  설치본 자체를 변경하지 않았으며 대상은 생성한 합성 복제본뿐이다.

최초 집중 시험에서 “state 판독이 누락 blob도 거절할 것”이라는 가정이 실패했다. 그래서
일반 load의 의미를 바꾸는 대신 명시적 `ReadOnlySession::verify_snapshot_objects`를 추가했다.
이는 저장된 모든 Snapshot manifest와 참조 blob을 기존 안전한 객체 reader로 스트리밍하며
주소 digest를 대조한다. working tree를 관측하거나 recovery/cleanup을 수행하지 않는다.

첫 전체 실행은 시험용 임시 디렉터리 이름 충돌로 멈췄다. 이름 생성에 프로세스 내 카운터와
배타적 생성의 AlreadyExists 재시도를 넣고 기존 디렉터리를 덮어쓰지 않도록 수정했다.
최종 전체 Core는 **1,310 통과/0 실패/1 ignored**, 34개 결과 그룹이다. ignored 1건은
명시적 구형 바이너리가 필요한 시험이며 해당 바이너리를 지정한 별도 실행에서 **1건 통과**했다.
이관 집중 시험은 7건 통과, 구형 writer 시험까지 합쳐 8건이다. all-targets build,
배포/고지/Windows inventory 84건, `git diff --check`가 통과했다.

위 수치는 이전 복제본 리허설의 결과이며 실제 Windows 실행·전원 중단·공간 고갈 검수가 아니다.
pending revisit·여러 Existence 조합, ACL/xattr 백업, 악의적 외부 writer 경합 및 신형 기록을
쓴 뒤 구형 백업으로 돌아가는 사용자 절차의 통합 검수는 추가로 필요하다.

### G5-D 원본 적용 개발 검수 (2026-10-13)

`src/visual_migration.rs`의 시험은 새로 만든 합성 프로젝트만 사용한다.

- 열린/닫힌 Verify에서 준비→상태 확인→적용→새 reader 판독. 기존 Report 의미, 작업 파일,
  객체, 미참조 파일과 빈 디렉터리 및 백업 bytes 보존. 같은 계획의 재적용 거절.
- 백업 부분 쓰기·file/directory sync·plan 발급 직전에 `ENOSPC` 주입. 원본 불변, 승인 가능한
  plan 미발급, 부분 백업을 다음 준비가 덮어쓰지 않는 것 확인.
- 적용 임시 파일 부분 쓰기·sync, rename 전후, 디렉터리 sync 이후, receipt 부분 쓰기 오류
  주입. 옛 state 또는 완전한 새 state만 남으며 검증된 원래 백업이 변하지 않는 것 확인.
- 별도 자식 프로세스를 부분 쓰기·rename 직전/직후·디렉터리 sync 이후에 실제 `SIGKILL`.
  부모가 잠금을 다시 얻어 상태를 읽음. 이는 실제 프로세스 강제 종료 시험이지 전원 차단이나
  물리 디스크를 가득 채우는 시험이 아니다. fault 주입과 강제 종료는 시험 코드에만 있다.
- stale 원본, 잘못된 승인 해시, 백업/복구본/후보 변조, busy, 링크·hardlink, 기존 임시 파일,
  프로젝트 안 백업 위치 거절. 최종 검사 직전 작업 파일·임시 파일 내용·잠금 교체도 거절.
- 이관 후 새 Verify 보고서와 PNG를 실제 Session으로 확정한 뒤 옛 계획 재적용 거절.
  추가된 새 보고서·이미지 객체·작업 파일과 원래 백업 모두 보존.

집중 시험 **8 통과**, 강제 종료용 자식 진입점 1개는 기본 ignored이며 부모 시험이 각
중단 지점에서 명시적으로 실행한다. 첫 컴파일에서 RuleSet 메서드명과 단위시험의 공용 fixture
crate 경로 오류를 수정했다. 검증 수치를 실제 사용자 데이터 이관이나 Host 표시 성공으로
확대하지 않는다.

최종 검수:

- `cargo test --locked -p gil`: **1,318 통과 / 0 실패 / 2 ignored**, 34개 결과 그룹.
  ignored는 위 자식 진입점과 명시적 구형 배포 바이너리가 필요한 시험이다. 자식은 부모
  시험 안에서 네 번 실행되며, 구형 writer 시험은 별도 실행하여 **1건 통과**했다.
- `cargo build --locked -p gil --all-targets`: 통과. 새 example도 빌드했다.
- JS: 97건 중 **93 통과 / 4 기존 skip**. 배포·고지·Windows inventory: **84 통과**.
  이번 결과는 Windows 프로그램의 실행이나 이미지 쓰기 검수는 아니다.
- 실제 개발 example을 새 합성 폴더에서 실행: prepare가 plan 해시 발급 → status
  `not_applied` → apply 성공 → status `applied_state_present` → 재적용 거절(exit 1).
  단위시험 helper 호출만으로 명령 검수를 대신하지 않았다.
- 새 Rust 파일 rustfmt 검사 및 `git diff --check` 통과. 기존 형식의 저장 코드는 serializer를
  공유하도록 추출했으며 기본 Project 생성 형식은 계속 4다.

실제 사용자 프로젝트·기존 설치본·MCP 설정을 변경하지 않았다. 다음 단계는 이관한 합성
프로젝트의 과거 보고서와 새 이미지 보고서를 실제 Host에서 함께 읽고, 서버 재시작 후에도
동일하게 조회되는지 확인하는 종단 검수다. 그 성공도 Windows 원본 이관이나 사용자 자동
이관·배포 승인으로 확대하지 않는다.

### 이관 후 보고서·이미지와 서버 재시작 종단 검수 (2026-10-14)

`examples/visual_migration_host_fixture.rs`는 새 목적지만 받는 합성 검수 fixture다.
format 4의 Define·Hypothesis 보고서를 작성하고 Verify를 연 상태에서 개발용
prepare/apply를 실행한다. 그 뒤 Verify에서 640×300 합성 PNG를 생성해 Session close로
확정한다. 이는 사용자 인터뷰나 실제 성능 측정 자료가 아니며 실제 프로젝트를 복사하지 않는다.

- 실제 Codex의 기존 승인된 개발 MCP App에서 `C2/S1` 원문과 성공 조건이 표시됐다.
  과거 보고서의 시각 자료를 소급 작성하지 않았고 미기재 안내가 표시됐다.
- `C2/S3`에는 새 보고서·파란 막대 네 개·caption·용도·작성자 기재 출처·원래 Verify가
  표시됐다. DOM의 `complete=true`, 실제 크기 640×300 및 화면 캡처로 확인했다.
  기존 보안 정책을 유지한 채 검증된 동일 PNG bytes의 data 표현으로 표시했다.
- 독립 stdio 검수에서 첫 서버 종료를 확인하고 두 번째 프로세스를 시작했다. 두 번째는
  prepare를 호출하지 않고 저장된 같은 scope를 복원했다. 서버 세대는 달라졌지만 기존
  Define/Hypothesis·새 Verify 상세, 시각 메타데이터·출처, Graph 사실이 모두 동일했다.
- 공용 App 이미지 조립 코드로 다시 읽은 PNG 1,301 bytes가 원본과 완전히 일치했다.
  SHA-256: `850eab7501eff766fa2c6e466098f7245df51ef75f01adadaee411edf2078970`.
  재시작 검수 전후 프로젝트 전체 및 백업 전체 파일 목록·bytes·권한이 불변이었다.
- 새 example 빌드·실행·내부 assertion과 rustfmt, `git diff --check` 통과.
  Core 제품 로직과 설치본은 이번 검수에서 변경하지 않았다.

**Host 재연결 후 화면 검수(2026-10-14):** 사용자가 임시 `gil-visual-g4` 재연결을
완료한 뒤 기존 합성 프로젝트 카드를 새로고침했다. 새 카드 생성이나 prepare 재호출 없이
Define를 선택하여 과거 본문·성공 조건·시각 자료 미기재 안내를 확인하고, Verify로 전환하여
새 보고서·이미지·설명·용도·출처를 다시 읽었다. 이미지 로딩 완료와 실제 크기 640×300을
확인했으며 DOM에 로드된 PNG bytes의 SHA-256도 위 원본과 동일했다. 실제 화면 캡처를
로컬 검수 증거로 보존했다. 프로젝트와 백업의 전체 경로·bytes·권한 지문도 재연결 전과 같다.

연결 정보는 현재 모드 `fullscreen`을 보고했다. 이는 App 내부 화면 검수이며 Host 전체
채팅 입력창 동시 표시나 앱 완전 종료 검수를 대신하지 않는다. 화면에 서버 세대 식별자는
노출되지 않아 새 프로세스 증명은 위 독립 stdio 시험과 구분한다. 이번 확인은 사용자가
재연결한 뒤 기존 카드에서 보고서·이미지가 다시 조회된 범위다. Windows와 실제 사용자
이관도 이번 검수 범위 밖이다. 제품 코드·설치본·보안 정책은 변경하지 않았다.

## 5. 일반 사용자용 생성·승인 이관 계약 — 확정, 구현 전 (2026-10-14)

이번 확정은 제품 절차의 설계 승인이다. 설치본 교체·실제 프로젝트 이관·공개 게시 권한을
부여하지 않는다. 아래 MUST에 해당하는 조건을 모두 구현·검수하기 전 공개 staging의
작성 기능 차단을 해제하지 않는다. 기존 형식 번호 4/5와 Report 불변식은 유지한다.

### 5.1 지원 여부와 사용자에게 묻지 않을 것

- Agent가 설치된 Core의 capability, 도구 schema, 실제 OS/파일 시스템, 프로젝트 상태를
  확인한다. 사용자에게 CPU·format 번호·asset ID·plan 해시 입력을 요구하지 않는다.
- 첫 작성 후보 지원 범위는 검수된 macOS Apple Silicon 로컬 파일 시스템이다. Windows는
  현재 읽기/쓰기 파일 경계와 이관이 미검수이므로 이미지 기능 동등성을 주장하지 않는다.
  미지원 시 보안 설정을 끄거나 파일 링크를 제거하는 우회 설치를 안내하지 않는다.
- 사용자 선택 폴더가 정본이다. 경로가 없으면 Host 폴더 선택을 안내하고, 부모 프로젝트나
  최근 사용 폴더를 추측하지 않는다. 기존 Git 폴더도 새 GIL 프로젝트 대상일 수 있다.
  Git 초기화·커밋·push·ignore 수정은 생성/이관의 부수 동작이 아니다.
- 파일을 기록할 가치가 있는지 GIL이 대신 판단하지 않는다. 범위·민감 자료·지원하지 않는
  파일을 만났을 때 사실과 선택지를 설명한다. 제외 패턴을 조용히 추가하거나 파일을 삭제하지
  않는다. 이미지 형식 한도는 첨부 디코딩 한도이지 Artifact 자동 제외 기준이 아니다.

### 5.2 폴더 상태에 따른 분기

| 실제 확인 상태 | 허용되는 흐름 |
| --- | --- |
| 기존 GIL 흔적 없음, 생성 가능, 작성 capability 준비됨 | 사용자 시작 요청에 따라 처음부터 format 5 생성 |
| 정상 format 5 | 재초기화하지 않고 기존 여정/Monitor 연결; 현재 열린 자리에서 계속 |
| 정상 format 4 | 그대로 읽기/Monitor 가능; 이미지 쓰기를 원할 때만 §5.4 승인 절차 |
| 더 오래된/미지 형식, `.gil`만 남음, 읽기 거절·권한 부족·복구 흔적 | 없음으로 간주하지 않음; 원본 보존 후 원인과 필요한 조치를 설명 |
| 미지원 플랫폼/파일 시스템/도구 조합 | 이미지 생성·이관을 시작하지 않음; 지원 범위 안내 |

마지막 경우 기존 텍스트 기능을 계속 쓰는 것은 가능하나 이미지 가능한 새 프로젝트라고
보고하지 않는다. 이미지 시작 요청을 몰래 format 4 생성으로 대체하지 않는다. 사용자가
텍스트 전용 시작을 선택하는 경로는 기존 의미를 유지한다. 생성 경쟁은 잠금 아래 다시 확인한다.

### 5.3 새 프로젝트 — 한 번의 시작 요청

사용자가 고른 폴더에서 “GIL 프로젝트를 시작해 줘”라고 요청하면 지원되는 작성 후보는
format 5를 기본으로 사용한다. format 선택 질문이나 별도 ‘이미지 기능 켜기’ 승인을 반복하지
않는다. 이는 향후 작성 후보의 기본값이며 **현재 설치본의 format 4 기본값을 이번에 바꾼 것이 아니다.**

1. 생성 전에 짧게 알린다: “이 폴더에 GIL 기록을 시작하고 모니터를 열겠습니다. 검증한
   이미지도 보고서에 남길 수 있으며, 새 기록은 이미지 기능을 지원하는 GIL이 필요합니다.”
2. 명시된 루트에서 기존 흔적·잠금·범위·지원 여부를 확인한다. 기존 작업 파일과 Git 기록은
   고치지 않는다. 저장 성공 전 Monitor나 첫 질문이 이미 준비됐다고 말하지 않는다.
3. **최초 state부터 format 5로** 확정한다. 먼저 format 4를 공개한 뒤 자동 이관하는 우회는
   금지한다. 빈 시각 registry와 정당한 초기 경계를 만들며 가짜 Verify/자료를 만들지 않는다.
   최초 Interview Cycle 하나, 열린 Step 0개를 유지한다. 이후 새 Verify close에는 첨부,
   불필요+이유, 누락+이유/후속 계획 중 선언이 필수다.
4. 확정 후 prepare → show로 Monitor를 요청하고, 지원 Host에 가로/fullscreen을 한 번
   요청한다. 첫 Interview 질문을 연다. 화면 표시를 기다리는 동안 별도 Cycle은 열지 않는다.
   화면 실패는 프로젝트 생성 실패가 아니며 start를 재호출하지 않는다. Companion은 자동
   실행하지 않는다. 사용자의 inline 선택을 존중한다.
5. 시작 응답 유실 시 정확한 폴더를 읽어 ‘기록 있음/없음/판정 불가’를 구분한다. 이 답만으로
   이번 호출이 생성했다고 단정하지 않는다. 없음이 검증됐을 때만 새 시작을 다시 제안하고,
   남은 객체·임시 파일·잠금을 삭제하는 정리는 별도 진단 없이 하지 않는다.

생성 승인은 모든 화면 캡처·민감 자료 저장·외부 전송 승인이 아니다. 첫 이미지 저장 전에는
작성 계약 §2의 보존/Host 전달 안내를 하고, 작업 범위 밖 자료에는 별도 권한을 확인한다.

### 5.4 기존 프로젝트 — 준비와 원본 적용의 두 승인

단순 open/status/context/Monitor/Plugin 업데이트는 이관을 일으키지 않는다. format 4에서
이미지 요청이 오면 현재 여정을 확인하고 아래 순서로 진행한다. 승인 거절은 정상 선택이다.

**A. 읽기 전용 확인과 준비 승인**

Agent는 선택 폴더·현재 형식·열린 Cycle/Step·복구 필요 여부를 확인하고 다음처럼 묻는다.

> 이 프로젝트에서 이미지 보고서를 쓰려면 기록 형식을 한 번 바꿔야 합니다. 기존 보고서와
> 진행 중인 작업은 유지합니다. 먼저 [백업 위치]에 기록과 작업 파일을 복사해 보존 여부를
> 검사할까요? 아직 원본은 바꾸지 않습니다. 전환 뒤에는 옛 GIL로 이 기록을 열거나 쓸 수 없습니다.

백업 위치는 프로젝트 밖의 새 로컬 비공개 폴더다. Agent가 적절한 위치와 예상 공간을 제안하되
사용자 승인 및 Host 파일 접근 권한 범위 안에서만 쓴다. 위치 권한이 없으면 선택 UI로 받는다.
클라우드 동기화/공유 저장소로 몰래 복사하지 않는다. 데이터 복제가 발생하므로 읽기 조회와
구분한다. 사용자가 거절하면 원본 format 4를 유지하고 이미지 확정을 중단한다.

**B. 백업·후보·복구본 검증**

§1–3의 no-follow/잠금/동기화/전체 digest 검증을 제품 도구 안에서 수행한다. 사용자에게
Cargo·개발 example·직접 YAML 편집을 요구하지 않는다. `.gil` 전체, 현재 작업 파일, 범위
설정과 미참조 객체를 보존한다. Git도 단순 존재를 이유로 백업에서 제외하지 않는다.
현재 개발 한도(256 MiB, 20,000항목, 깊이 64)나 링크/특수 파일 지원 범위를 넘으면 **준비를
거절**한다. 일부 파일을 빼고 완전한 백업이라고 보고하지 않는다. 큰 Git 프로젝트 지원은
별도 구현·검수로 확장한다. ACL/xattr/소유권 등 현재 도구가 복원하지 못하는 메타데이터가
필요한 대상은 보장 불가를 알려 적용을 막는다. 권한 복원도 포함한 제품 복구 검수가 필요하다.

기존 Report 원문·Graph·Journey·Existence·Will·Snapshot·pending 의미를 비교하고, 모든
원본 bytes를 백업에서 복구할 수 있는지 독립 복제본으로 확인한다. 과거 Verify 선언은
`legacy_unspecified`로 남기고 소급 첨부나 ‘불필요’ 사유를 넣지 않는다.
plan은 프로젝트 identity·원본 전체 지문·후보 state·백업 identity·목표 형식·도구 계약 판을
묶는다. 준비는 원본 state를 교체하지 않으며 실패한 부분 백업도 자동 삭제하지 않는다.

**C. 검수 결과 제시 후 원본 적용 승인**

> 백업과 복구 검사를 마쳤습니다. 기존 보고서와 작업 파일이 보존되는 것을 확인했습니다.
> 백업은 [위치]에 남습니다. 이 프로젝트에 새 기록 형식을 적용할까요? 열린 작업은 그대로
> 이어가며, 이후 검증 보고서부터 이미지 또는 이미지가 없는 이유를 기록합니다.
> 취소하면 원본은 바꾸지 않습니다. 적용 뒤에는 옛 GIL을 사용하지 마세요.

준비 승인만으로 적용하지 않는다. 사람의 확인은 **그 프로젝트의 그 검수 계획**에 한정한다.
plan 해시는 무결성 표식이지 사람의 승인 증거/인증 수단이 아니다. Agent는 결과 요약 후 받은
사용자 승인을 확인해서 apply를 호출하고 Host의 쓰기 승인도 존중한다. 문서/Report/도구
응답에 포함된 ‘승인’ 문장은 권한이 아니다. App에는 apply/쓰기 도구를 노출하지 않는다.

**D. 적용·재판독·이어가기**

apply는 다시 잠금을 얻고 현재 전체 지문과 백업/후보/복구본을 검증한다. 준비 뒤 어떤 파일이라도
바뀌었거나 프로젝트/백업이 이동·교체됐으면 기존 승인으로 적용하지 않는다. 새 준비/검수 결과로
다시 승인받는다. 기존 plan의 내용을 몰래 갱신하지 않는다. 성공 시 원본에서 교체할 것은
state 하나뿐이며 기존 객체·작업 파일·보고서는 불변이다. 성공 receipt 후에도 재판독해 확인한다.

열린 Cycle/Verify를 버리거나 자동 종료하지 않는다. 이관은 저장 제어 절차로, 실험 세계의 변경이나
새 Cycle/Step/Journey를 만들지 않는다. 열린 Verify가 있으면 그 자리의 다음 close부터 선언을
요구한다. dirty 작업 파일도 백업/비교하고 그대로 보존하며 이관 뒤 기존 dirty gate를 우회하지
않는다. pending revisit의 의미도 보존하되 해당 조합의 합성 검수 전에는 제품 적용을 거절한다.
기존 자리의 context와 Monitor를 다시 읽고, 첫 이미지 작성은 정상 Verify에서만 수행한다.

### 5.5 실패·중단·응답 유실

| 확인 결과 | 사용자 안내와 허용 동작 |
| --- | --- |
| 백업 중 용량 부족/권한 거절/취소 | 준비 미완료. 원본 보존, 부분 백업 보존. 완전한 백업·원본 적용이라고 말하지 않음 |
| 적용 전 지문 불일치/busy | 적용 중단. 파일·잠금 삭제 금지. 쓰기 주체 정리 또는 새 계획 필요 |
| 적용 응답 유실/rename 뒤 sync 실패 | 적용 여부 불명. apply 자동 반복·다운그레이드 금지, read-only status로 확인 |
| status가 준비한 옛 state와 일치 | 적용 안 됨. 전체 지문/승인 유효성을 재검사한 뒤에만 재시도 가능 |
| status가 준비한 새 state와 일치 | 적용 state 존재. receipt만 보지 말고 객체/보존 검사를 마친 뒤 이어감 |
| 다른 state/판독 불가/새 보고서가 생김 | 정상 추가 기록일 수도 있음. 모두 보존하고 멈춤; 옛 백업 덮어쓰기 금지 |

한정된 백업 공간 계산은 실제 write/sync 성공의 보장이 아니다. 잠금은 외부 편집기나 동기화를
멈추지 못하므로 준비/적용 중 중지를 안내하고 변화가 발견되면 거절한다. 백업 삭제는 자동
보관 만료/업데이트 정리와 묶지 않는다. 별도 삭제 요청 없이는 보존한다.
구형 프로그램으로 돌아가려면 최신 원본도 보존하고 옛 백업을 **다른 위치에** 복구해 검증한다.
새 기록을 포함한 원본에 덮어쓰거나 format 숫자를 낮추는 자동 복원 경로는 제공하지 않는다.

### 5.6 호환성·도구 계약의 제품 목표

지원되는 작성 배포 후보는 descriptor에서 format 4..5 지원을 사실대로 선언하고 최초 생성은
5를 사용한다. protocol/descriptor schema는 의미 변경이 없는 한 1을 유지하되, typed close와
승인 이관 동작이 포함된 **action surface는 2**로 식별한다. 현재 `visual-authoring` 빌드에
적용했으며 macOS arm64에서만 저장 범위 4..5를 선언한다. 기본 빌드는 surface 1/format 4를
유지한다. 미검수 플랫폼의 feature 빌드는 새 도구 표면을 2로 구분하되 저장 범위를 넓히지 않는다.
기존 format-4 텍스트 호출은 유지한다. 구형 consumer가 1만 받는다면 명시적 비호환으로 멈추게
하고 몰래 1로 위장하지 않는다. update policy/Skill/패키징/이전 실제 binary 시험을 함께 갱신한다.
읽기만 되거나 플랫폼 파일 안전성이 미검수인 빌드는 위 완전한 작성 capability를 광고하지 않는다.

Agent 제품 도구는 **비수정 사전 확인 / 백업 준비 / 승인된 계획 적용 / 비수정 상태 확인**을
구분한다. 준비는 백업 쓰기, 적용은 원본 쓰기이므로 readOnly로 표시하지 않는다. raw 전체
백업/절대 경로/이미지 bytes를 모델 응답에 불필요하게 싣지 않는다. 대상과 백업 위치를 설명하는
데 필요한 정보는 제공한다. 구체적인 CLI/MCP 이름·JSON schema·승인 연결은 구현 직전 별도
wire 계약으로 확정하며, 이 절차 문서의 표현을 이미 설치된 명령으로 안내하지 않는다.

### 5.7 인수 게이트와 구현 순서 — 실행 증거는 §6 이후에 별도 기록

| 게이트 | 최소 인수 시나리오 |
| --- | --- |
| N1 신규 생성 | 빈 폴더/기존 Git 폴더, 최초 format 5, Interview 하나/열린 Step 0, 기존 파일 불변 |
| N2 생성 실패 | 이미 4/5 존재, `.gil` 잔여물, 권한 거절, 동시 start, commit 전후 중단·응답 유실 |
| N3 화면/작성 | start→Monitor→첫 질문, 화면 실패 시 재초기화 없음, 첫 Verify의 첨부/없음/누락, 재사용 |
| M1 승인 | 준비 거절, 적용 거절, 승인 전 apply 차단, 다른 Project/plan 승인 전용 금지 |
| M2 보존 | 닫힌 Cycle/열린 Verify/dirty 파일/한글·공백·개행 보고서/미참조 객체/권한 복구 |
| M3 제한 | 큰 Git 프로젝트·용량 한도·지원 못하는 메타데이터·링크·특수 파일은 제외 없이 명시적 거절 |
| M4 경쟁/복구 | 준비 뒤 파일·루트·백업 변경, busy, ENOSPC, 강제종료, 응답 유실, 새 기록 후 옛 계획 재적용 거절 |
| M5 실제 버전 | 옛 binary의 format 5 쓰기 거절/bytes 보존, format 4 호환, descriptor·schema·업데이트 판정 일치 |
| M6 Host | 이관 전후 기존 보고서·새 이미지·설명·출처·재시작 보존, 사람의 화면 인수 |

순서: **N1/N2 생성 Core → N3 도구/Skill 합성 검수 → 이관 wire/승인 계약 → M1–M5 합성
제품 도구 검수 → M6 격리 설치 Host 인수 → 별도 승인된 실제 사용자 이관/후보 게시**.
기존 개발 도구의 합성 통과 증거는 재사용하되 위 제품 경로의 전체 통과로 세지 않는다.
Windows는 native 경계가 닫힌 뒤 같은 인수표를 적용하며 macOS 결과로 대신하지 않는다.

### 5.8 승인 이관 MCP wire (2026-10-14, 개발 후보)

`visual-authoring` 개발 빌드에만 다음 네 도구를 연결한다. 일반 설치본/default 도구와
기본 descriptor는 바꾸지 않는다. 개발 descriptor는 §5.6의 surface 2를 따른다.
현재 실행 범위는 macOS arm64 로컬 디스크의 합성 Project다.
공개 배포/실제 사용자 이관/M5 전체 호환 인수를 뜻하지 않는다. App visibility는 `model`만,
모든 입력은 추가 필드를 거절한다. CLI를 새로 만들거나 shell로 우회하지 않는다.

| 도구 | 필수 입력 | 성공 응답 / 부작용 |
| --- | --- | --- |
| `gil_migration_check` | `project_root` | 형식·열린 Cycle/Step·전체 bytes/항목·보수적 백업 용량 추정, 비수정 |
| `gil_migration_prepare` | `project_root`, `backup_root`, `backup_approved` | 백업/후보/독립 복구 검수 뒤 `plan_sha256`, 원본 미변경 |
| `gil_migration_apply` | 위 두 경로, `plan_sha256`, `apply_approved` | 해당 계획 재검사 후 state만 원자 교체, 보존 재판독 |
| `gil_migration_status` | 위 두 경로, `plan_sha256` | state 분류와 전체 보존 판정, 비수정 |

승인 필드는 boolean이며 false는 파일 생성 전에 `approval_required`로 거절한다. 누락/잘못된
타입/추가 필드/비정규 해시는 invalid params다. true는 **Agent의 사람 승인 확인 선언**이지
서버가 사람을 인증했다는 뜻이 아니다. prepare 결과를 요약하고 그 계획에 대해 새로 받은
승인만 apply로 전달한다. 준비 전 받은 포괄적 승인을 적용 승인으로 쓰지 않는다.
경로+plan digest는 범위/무결성 검증이며 인증 토큰이 아니다. 외부 writer 중지는 사용자와
조율해야 하며 Host 권한 확인은 유지한다. 승인 거절은 정상 종료이며 새 백업/재시도는 자동화하지 않는다.

계획 v2는 백업의 정규 경로·device/inode·목표 format 5·계약 판을 함께 묶는다. 백업은
0700/0600, 독립 복구본은 private 상위 폴더 안에서 원래 mode로 복원/재판독한다. 보존 범위는
bytes/빈 디렉터리/mode이며 timestamp는 복원 보장이 아니다. macOS가 자동 생성하는
`com.apple.provenance`는 원본·백업·복구본·교체 state의 값까지 대조하며 삭제/무시는 하지 않는다.
복제 시 값이 달라지면 적용하지 않는다. 다른 xattr, ACL, 특수 flags,
현재 사용자와 다른 소유권, 접근 불가능 mode, pending revisit는 검수되지 않아 거절한다.
공유/동기화 폴더 탐지는 완전하지 않으므로 백업 위치가 개인 로컬 폴더인지 승인 시 확인한다.

실패 응답은 내부 파일 내용/불필요한 경로를 내보내지 않는다. prepare 실패는 부분 백업을
보존한다. apply 실패/응답 유실은 결과 불확실일 수 있으므로 status만 읽고 자동 재적용하지 않는다.
status의 `preserved: true`는 계획의 전체 목록에서 state만 허용된 값이고 다른 bytes/mode가
같다는 뜻이다. 이후 정상 작업도 지문을 바꾸므로 false만으로 손상 판정을 하지 않는다.

## 6. N1/N2 신규 이미지 Project 생성 Core (2026-10-14)

`visual-authoring` 개발 feature에 `ProjectSession::start_visual(rules, absolute_root)`를
추가했다. macOS Apple Silicon의 로컬 파일 시스템에서만 진행한다. **성공 반환 시 최초
state까지 저장되어 있고 Session 잠금은 유지된다.** 기존 `ProjectSession::start`와 달리
별도 최초 commit을 요구하지 않는다. CLI/MCP의 `start`, Agent Skill, descriptor, 설치본의
기본값은 이번에 바꾸지 않았다. 사용자 도구 연결은 다음 N3 단계다.

- 선택한 절대 루트에서 `.gil` 디렉터리를 배타적으로 만든다. 이미 있는 format 4/5,
  미지 형식, 빈 디렉터리, 링크, 미완 객체/임시 파일도 재사용하지 않는다. 기존 디렉터리의
  잠금을 새로 만들거나 recovery/reap를 실행하기 전에 거절한다.
- 처음 관측한 세계로 Interview 하나·Step 0개를 만든다. 메모리에서 빈 시각 registry와
  초기 경계를 세우고 최초로 직렬화하는 state부터 format 5다. 디스크에 format 4를 쓴 뒤
  자동 이관하는 경로는 없다. 기존 작업 파일·Git 설정·ignore는 수정하지 않는다.
- 루트 handle 기준 디렉터리 생성, 잠금/디렉터리 identity 재검사, 배타적 임시 파일,
  file sync/재판독, macOS `RENAME_EXCL` 최초 공개, directory sync를 사용한다.
  최초 저장 직전에 다른 state가 나타나면 그것을 덮어쓰지 않는다.
- 오류 뒤 임시 파일·객체·디렉터리를 자동 삭제하지 않는다. 공개 뒤 실패하면 완전한 새
  state가 남을 수 있다. 오류가 곧 ‘프로젝트가 없다’는 뜻은 아니며 읽기 확인 없이 재시작하지
  않는다. 동일 루트로 다시 생성하면 흔적 존재 때문에 거절한다.

### 합성 검수

집중 Core 단위시험 **9 통과**, 공개 library 진입점 통합시험 **1 통과**. 별도 자식 진입점
1개는 일반 목록에서 ignored이며 부모 시험이 **5회 실제 실행하여 SIGKILL**했다.

- 빈 폴더와 합성 Git 폴더: 첫 공개 전 임시 state도 format 5, 초기 Interview 하나·Step 0,
  빈 registry, 기존 파일 bytes/권한 보존, 저장 후 읽기 확인.
- 기존 4/5·미지 형식·빈 `.gil`·중단 흔적·파일/링크 충돌: 전체 경로/bytes/권한 불변.
- 동시 생성 두 개 중 하나만 성공. 상대 경로·없는 폴더·파일·실제 쓰기 권한 거절 검사.
- 디렉터리·잠금 교체, 기존 임시 파일, 최초 공개 시 다른 state 생성: 거절 및 대상 보존.
- Artifact 관측 중 지원하지 않는 링크 거절: 링크·원본 파일·남은 생성 흔적 보존.
- 예약·capture 후·encode 후·부분 쓰기·file sync 후·공개 전후·directory sync 후·완료 응답
  직전의 **9개 checkpoint에서 ENOSPC 오류 주입**. 공개 전에는 state가 없고, 공개 후에는
  완전한 format 5가 읽혔다. 모든 경우 원래 작업 파일과 재시도 전후 잔여 기록을 보존했다.
- 예약 후·부분 쓰기·공개 직전/직후·directory sync 후에 자식 프로세스를 강제 종료했다.
  공개 후의 읽기 전용 재개는 잠금을 다시 얻었고 기록을 변경하지 않았다.
- 새 프로젝트를 Interview→첫 Verify로 진행: 시각 선언 없는 close 거절, 불필요+이유로
  정상 close/저장, 재판독 및 중복 생성 거절 뒤 보고서 bytes 보존.

최초 컴파일에서 테스트의 Graph 접근자 이름 두 개를 바로잡았고, 로컬 파일 시스템 검사
추가 때 `MNT_LOCAL` 상수의 정수형 변환을 수정한 뒤 통과했다. 오류 주입은 checkpoint에서
반환한 오류이며 물리 디스크를 채운 시험이 아니다. SIGKILL도 실제 전원 차단을 대신하지 않는다.
기존 Artifact 저장 경계 전체, 악의적 외부 writer의 모든 경합, Windows/Intel/네트워크 파일
시스템의 안전성을 새로 보장하지 않는다. 이번 게이트는 library 생성 경로의 합성 검수이며
Agent 도구 연결·Host 화면·완전한 제품 capability·공개 배포 인수는 아니다.

### 전체 회귀 및 보존 확인

- `cargo test --locked -p gil`: **1,318 통과 / 0 실패 / 2 ignored**.
- `cargo test --locked -p gil --features visual-authoring`: **1,332 통과 / 0 실패 /
  3 ignored**. 추가 ignored는 위 강제 종료 자식 진입점이며 부모가 명시적으로 실행했다.
  두 명령은 shared target의 feature 혼선을 피하려 순차 실행했다. 각 35개 결과 그룹이다.
- 기본/작성 feature의 `cargo build --locked -p gil --all-targets`: 모두 통과.
- UI·MCP App·Plugin·Codex 배포·고지 JS 합산 **183건 중 179 통과 / 4 기존 skip**.
  Windows native 실행이나 새 작성 후보 packaging/설치 검수를 의미하지 않는다.
- 변경 Rust 파일의 rustfmt 및 `git diff --check` 통과. 기존 검수 Project 파일과
  사용자 config/설치된 preview.3 Skill의 보존 지문 **10개 불변**을 확인했다.

기존의 다른 수정은 보존했다. 실제 사용자 Project, Plugin 설치·설정·패키지, 공개 staging은
변경하지 않았으며 커밋·push·CI dispatch·게시도 하지 않았다.

## 7. N3 Agent 시작 도구 연결 (2026-10-14, 개발 후보)

### 개발 wire 및 작성 지침

- `visual-authoring` feature의 **`gil_start`**만 §6의 `start_visual`에 연결한다.
  입력은 기존 `project_root` 하나를 유지하며 절대 경로·존재하는 폴더 검사를 통과해야 한다.
  서버 cwd나 부모를 선택하지 않는다. 기본 빌드와 CLI `gil start`는 기존 format 4 경로다.
- 개발 tool description은 신규 format 5, macOS Apple Silicon 로컬 지원, 기존 기록/흔적
  보존, 미지원 거절을 설명한다. 기존 `ok/exit_code/said/problem` 응답을 유지하고 생성 성공에만
  `storage_format: 5`, `display_confirmed: false`를 더한다. 도구 수나 UI 쓰기 권한은 늘리지
  않는다. 이것은 완전한 제품 capability/action surface 2 승격이 아니다.
- 이미 있는 정상 4/5는 잠금·읽기 전용 판독 뒤 ‘이미 걷고 있다’로 거절한다. 이를 기존
  여정을 이어갈 근거로 삼되 재초기화하지 않는다. 미지 형식·잠금 없는 기록·부분 흔적·권한
  거절은 ‘없음’으로 판단하지 않고 보존한다. 손상된 파일을 지우라는 안내를 이 경로에 넣지 않는다.
- 시작 오류는 확정된 ‘아무것도 생성하지 않음’ 응답이 아니다. 공개 뒤 응답 유실 가능성이
  있으므로 자동 재호출하지 않는다. 개발 후보의 `gil_status`/`gil_context`는 recovery/reap
  없는 ReadOnlySession을 사용한다. `.gil` 흔적만 있고 state/lock이 없으면 lock도 만들지
  않고 불명으로 거절한다. 정상 기록은 기존 공용 renderer로 읽는다.
- 저장소 후보 Skill은 tool description 기반 생성 안내, start→prepare→show→첫 Interview,
  화면 미확인과 생성 성공의 분리, 시작 재시도 금지 및 기존 기록 읽기 흐름을 따른다.
  기존 설치된 Skill은 교체하지 않았다. 이미지 생성 안내는 민감 캡처/전송 승인이 아니다.

### 합성 stdio 검수 범위

실제 새 MCP 자식 프로세스를 사용한다. 기존 설치본·Host 설정이나 실제 사용자 프로젝트는
연결하지 않는다. Interview에서 Verify로 가는 준비 보고서는 합성 fixture이며 실제 사람의
인터뷰 승인이나 독립 Agent 실행을 대신하지 않는다.

- `gil_start`의 최초 format 5 → prepare/show의 열린 Interview 하나·Step 0 → `gil_open`
  첫 질문 → View 갱신과 live OS watcher hint. 생성만으로 Monitor binding을 만들지 않으며
  show 응답을 실제 fullscreen 성공으로 세지 않는다.
- 기존 4/5·미지 형식·부분 state·잠금 없는 기록에서 시작 거절과 status/context 조회 뒤
  모든 기존 파일 bytes가 유지된다. 미완 Artifact 객체를 읽기 과정에서 삭제하지 않는다.
- 시작 응답을 잃은 것으로 취급해 서버를 종료하고 새 프로세스에서 status→prepare/show를
  호출한다. 최초 Interview가 그대로 있고 새 Cycle/Step이나 state 변경은 없다.
- 새 도구로 만든 Project의 첫 Verify에서 선언 없는 close와 저장 충돌을 거절한다.
  `attached`/`not_needed`/`missing`의 세 선언을 각각 확정한다.
- 새 도구로 만든 Project에 PNG 첨부 → 서버 재시작 후 확정 조회 → Analysis/Outcome/
  최종 Cycle/다음 Define에서 재사용 → 원본 PNG 삭제 후 Monitor 이미지 bytes 조회를
  기존 stdio 작성 시험으로 연결했다. 기존 format 4의 첨부 거절도 유지한다.

첫 sandbox 실행에서는 21건 중 20건이 통과했으나 OS watcher hint 시험이 실패했다.
실제 OS 감시 환경에서 재실행하여 live 분기와 22건 통과를 확인했다. 감시 코드는 수정하지
않았다. 읽기 전용 연결 추가 중 renderer가 요구하는 Session 타입의 컴파일 오류를 수정했다.
Skill validator와 문구 계약 시험 12건이 통과했다. 이는 Skill의 실제 Agent 실행 인수와는 다르다.

남은 게이트: 실제 Host/격리 Agent의 새 시작 인수, 승인 이관 wire와 M1–M5 제품 도구,
최종 호환성 descriptor/패키징 정합, 별도 승인된 후보 게시. 기존 G4/G5 화면 확인을 이번
새 시작 후보의 실제 화면 확인으로 대신하지 않는다. Windows는 새 생성 지원으로 광고하지 않는다.

### 최종 회귀

- 기본 Core **1,318 통과 / 0 실패 / 2 ignored**, 개발 작성 Core **1,333 통과 / 0 실패 /
  3 ignored**, 각각 35개 결과 그룹. 기본/작성 all-targets build 통과.
- 작성 전체 suite의 첫 실행에서 강제 종료 자식이 `kill(SIGKILL)` 반환 뒤 다음 쓰기까지
  진행할 수 있는 시험 경합으로 1건 실패했다. 종료 요청 뒤 park하여 다음 쓰기를 실행하지
  않도록 자식 시험 코드만 수정하고 전체 작성 suite를 다시 실행해 통과했다. 제품의 저장
  로직을 이 실패 때문에 변경하지 않았으며 실제 전원 차단 검수라고 주장하지 않는다.
- JS/UI/Plugin/배포/고지 **183건 중 179 통과 / 4 기존 skip**. Skill validator 통과.
- 기존 설치 Skill/설정 및 앞선 합성 검수 기록의 지문 **10개 불변**. `git diff --check` 통과.

기존 설치본 교체·사용자 기록 변경·자동 이관·커밋·push·CI·공개 게시는 하지 않았다.

## 8. N3 격리 Agent 신규 생성·작성 인수 (2026-10-14)

새 `visual-authoring` release 패키지를 만들고, 이전에 사용자가 로그인한 **격리 Codex
프로필만** 공식 Plugin 관리 명령으로 교체했다. 일반 설치본은 그대로다. 패키지는
`publishable: false`, 도구 20개, 압축 왕복 및 stdio smoke 통과다. 이번 binary SHA-256은
`9f6457d8baab3d0ec714a2cc4a8916ff0062c254c47d5617cb895c8bb277f569`, archive는
`8c96cc593d0f8104283087852b5b0706f1c1425e619d6bb3fabc0e450bc21c77`이다.
개발 manifest의 version 문자열은 이전 후보와 같으므로 이것을 공개 버전 승격으로 세지
않으며, 설치된 binary/Skill bytes와 새 패키지의 동일성을 별도로 검사했다.

### 실제 Agent 실행과 보존

- 빈 합성 폴더에서 실제 `codex exec` 두 턴을 실행했다. 각 턴에서 후보 Skill을 자동으로
  읽었다. 첫 턴의 `gil_start`는 **한 번**, 생성 receipt는 `storage_format: 5`,
  `display_confirmed: false`이고 첫 질문을 연 뒤 답을 기다렸다. 이관 helper는 쓰지 않았다.
- 검수자가 제공한 합성 답변(A=40 파랑, B=70 주황, 외부 데이터/캡처 없음)을 근거로
  Interview를 닫고 실험 하나를 열었다. 소스는 열린 Verify에서 작성했으며, 실제 PNG
  1200×700, 8,649 bytes를 생성·디코딩·CRC/픽셀 검사했다. 생성 이미지라고 출처를 남겼다.
- Verify `step:C2/S3`에 첨부하고 최종 `cycle:C2`에서 같은 자산을 다른 caption으로
  인용했다. registry 자산은 하나이며 SHA-256은
  `24c52a0d8c259d95815c815532eda2b2a9e6d54cd37973edc952d659e1a87569`다.
  두 Cycle은 닫혔고 다음 Cycle은 열지 않았다. 원본 PNG도 남겼다.
- 새 패키지의 서버를 두 번 새로 시작해 두 보고서의 digest/이미지 metadata를 Agent의
  확정 receipt와 대조했다. 모두 동일하고 합성 Project 8파일은 판독 전후 bytes 불변이다.
  일반 설치 Skill/설정 및 이전 검수 자료의 보호 지문 10개도 불변이다.

### 첫 시도와 품질 한계 — 무실패 실행으로 기록하지 않음

- 첫 프롬프트의 포괄적인 ‘설정 변경 금지’ 때문에 Agent가 prepare의 로컬 기억 저장까지
  거절했다. 검수용 `GIL_MONITOR_STATE_DIR`에 한해서 허용한다고 두 번째 턴에서 정정했다.
  start를 재호출하지 않고 prepare/show 각 한 번으로 이어갔다. 따라서 이번 실행은
  두 턴의 기능 인수이며 무개입 한 문장 UX 재인수가 아니다.
- 도구 거절은 7건이다. 종류 미지정, basis_refs 형식, 열린 Step 중 다음 open, Cycle
  종료 필드/방향 오류를 Agent가 안내에 따라 수정했다. 시각 Cycle close의
  `close_not_confirmed` 뒤에는 확정 조회에서 `confirmed: false`를 확인하고 status/help를
  읽은 뒤 수정했으며 무조건 close를 반복하지 않았다. 기존 보고서를 덮어쓰지 않았다.
- Verify 본문의 ‘이미지 열람’ 표현은 실행 JSONL의 PNG 디코딩/픽셀 검사만으로는
  뒷받침되지 않는다. 별도 이미지 열람 이벤트가 없어 **작성 Agent의 시각 검사 여부는
  미확인**이다. 부모 검수자의 원본/Monitor 이미지 확인을 소급해 작성 Agent의 관측으로
  간주하지 않는다. 닫힌 보고서는 고치지 않고, 관측 방법을 구분하는 작성 지침/후속
  Agent 검수 항목으로 남긴다. 저장 기능 통과가 보고서의 모든 서술을 보증하지 않는다.

### 실제 Codex Monitor 범위

일반 설치본/주 프로필을 바꾸지 않기 위해 이미 연결된 **임시 작성 검수 서버를 읽기 경로로**
사용했다. 새 후보의 writer와 이 reader는 동일한 합성 Project를 읽으며 UI HTML digest는
기존 검수본과 같은 `6401f98113651a0454665d1c6ab4090ca5936de61c02d382f02f5909c7c1a6c8`이다.
이는 새 binary를 Desktop Plugin으로 교체한 end-to-end 설치 인수와는 다르다.

사용자가 새 카드를 펼친 뒤 실제 App의 fullscreen 상태·가로 그래프와 진행 갱신을 확인했다.
Verify와 최종 Cycle 선택에서 이미지/설명/출처 DOM 및 PNG 1200×700 로딩을 확인했고,
Verify 원본 크기 확대의 실제 화면도 확인했다. 확대 후 작업 진행 중 미리보기 상태로
바뀌어 복귀 버튼 탐색은 실패했다. 사용자 조작인지 갱신 영향인지 이 실행만으로 확정하지
않으며 확대 유지성을 별도 통과로 세지 않는다. Agent 작업이 끝난 뒤 최종 Cycle 이미지의
확대→미리보기 복귀는 다시 실행해 정상 확인했다. 2026-10-14 사용자가 C2 최종 사이클
보고서에 대해 “이미지·설명·출처와 채팅이 함께 잘 보여”라고 확인했다. 이 합성 Project의
최종 보고서 가독성·채팅 동시 사용은 사람의 화면 인수까지 완료했다. 이는 작성 당시 Agent의
시각 검사 여부나 새 Desktop 설치 전체 인수를 소급해 증명하지 않는다.
보안 정책을 느슨하게 하지 않았으며 기존 검증 PNG data 경로를 쓴다.

이번 턴의 제품 코드/Skill 변경은 없다. 개발 패키지 생성과 실제 Agent/재시작/Host 검수를
추가했으며 N3 이전 전체 회귀 수치를 새 실행으로 재집계하지 않는다. 승인 이관 wire,
M1–M5 제품 경로, descriptor와 공개 배포 정합은 계속 미완이다.

## 9. 승인 이관 Agent 도구 연결 (2026-10-14)

§5.8을 먼저 확정하고 `src/mcp/migration.rs`와 `migration-tools.json`에 네 도구를 연결했다.
일반 빌드는 19개 그대로, `visual-authoring` 개발 빌드는 기존 20개에 승인 이관 4개를 더한
24개다. 실제 설치본·Codex 주 설정·기존 사용자 Project·닫힌 Report는 변경하지 않는다.
Skill의 별도 승인 흐름은 도구 가용성과 합성 프로젝트 범위로 제한한다. 이미지 close의
`visual_upgrade_required` 거절을 자동 이관 승인으로 사용하지 않는다.

### 구현과 검수 범위

- 사전 확인은 기존 잠금 아래 판독·Snapshot 전수 검사·전체 파일 범위 검사를 하며 복구나
  cleanup을 실행하지 않는다. 열린 Cycle/Step은 유지한다. Cycle/Step pending revisit는
  해당 조합의 승인 이관 인수 전까지 거절한다.
- 기존 코어의 모드 장부만으로는 §5.4의 권한 복구 검수가 충분하지 않았다. 독립 복구본에
  원래 파일/디렉터리 mode를 복원하고 대조하도록 보완했다. 원본 state 교체도 원래 mode를
  유지한다. 계획 v2는 원본/백업 identity와 경로, 목표 형식, 원본 전체와 provenance를 묶는다.
  이전 개발 계획 v1을 몰래 새 계획으로 승인하지 않는다.
- 준비/적용 승인은 별도 boolean 확인 선언이며 false/누락/잘못된 schema는 쓰기 전 거절한다.
  실제 사람의 대화 승인을 서버가 인증하는 것은 아니다. Host 승인+Agent 지침이 인간 경계다.
- 실제 stdio 자식 프로세스 시험 하나에서 열린 Verify와 닫힌 Cycle 각각을 대상으로
  준비 거절/누락 → 읽기 사전 확인 → 백업/후보/복구본 검증 → 적용 거절 → 잘못된 해시/다른
  Project 거절 → 새 파일 drift 거절 → **새 백업 계획과 모의 재승인** → 적용 → 서버 재시작
  후 상태/보존 조회를 실행했다. 기존 Graph/Journey/Report 등 YAML 값(format/visuals 제외),
  미확정 파일, `.git`, 미참조 파일, bytes와 실행 권한이 보존됐다. 원본은 state만 교체됐다.
- 백업 이동/복제본 바꿔치기, ACL·미지원 xattr, 한도 초과 Git sparse 파일의 준비/적용을
  거절하고 원본·메타데이터를 남겼다. 기존 busy/unsafe path/최종 gate 경합, 쓰기·sync ENOSPC
  주입과 실제 SIGKILL, 적용 뒤 새 보고서/이미지에 옛 계획 재적용 거절 시험도 함께 회귀한다.
  실제 디스크를 꽉 채운 시험이나 물리적 전원 차단 인수는 아니다.
- 상태 분류와 전체 보존 여부는 같은 잠금 안에서 읽는다. 새 state가 있어도 뒤에 정상 작업이
  추가됐으면 `preserved: false`일 수 있다. 손상으로 단정하거나 원본 복원/자동 재적용하지 않는다.

초기 실패를 숨기지 않는다: macOS metadata trait import 컴파일 오류를 수정했고, xattr 전면
거절이 합성 파일의 자동 provenance까지 막아 값 대조 방식으로 보완했다. ACL 없음의 ENOENT를
정상 판정하되 실제 ACL은 계속 거절한다. 전체 최초 실행에서는 새 catalog 기대값 1곳 누락과
샌드박스의 loopback 제한을 구분해 수정/허용 환경 재실행했다.

이 검수는 **실제 MCP protocol + 모의 사람 승인**이다. 실제 모델이 새 승인 대화를 올바르게
수행하는지, 새 후보 Desktop 설치/화면 M6, Windows 원본 이관, descriptor/action-surface 2와
공개 배포 정합은 별도 미완이다. 기존 M6 화면 인수를 이 새 Agent 경로의 인수로 옮겨 세지 않는다.

### 최종 실행 장부

- `cargo test --locked -p gil`: **1,321 통과 / 0 실패 / 2 ignored**, 35 결과 그룹.
- `cargo test --locked -p gil --features visual-authoring`: **1,338 통과 / 0 실패 / 3 ignored**,
  35 결과 그룹. ignored 중 자식 강제종료 진입점은 부모 시험에서 별도로 실행한다.
- 명시한 실제 preview.3 binary로 구형 writer 거절 시험을 별도 실행: **1 통과**.
- 기본/개발 `cargo build --locked -p gil --all-targets` 모두 성공, warning 0.
- UI/MCP App/Plugin/배포/고지 JS 합산 **179 통과 / 0 실패 / 기존 skip 4**.
  Skill validator와 `git diff --check` 통과.
- 설치하지 않은 macOS 개발 패키지: archive 왕복·실행 비트·24개 tool schema/visibility smoke
  통과, `publishable: false`. archive SHA-256
  `f11fbf0808103fd77ae1896d8cb21dd3bfe8f615b40670cd301378258f4184db`.
- 그 **패키지의 binary**로도 합성 승인 흐름을 실행했다. 기존 preview.3 실행 파일을 수정하지
  않고 새 합성 폴더에 format 4/닫힌 질문 보고서를 만들고, 개발 패키지로 백업 거절→준비→
  적용 거절→모의 승인 적용→서버 재시작 상태 조회까지 통과했다. 원래 state는 백업에 byte
  그대로, 기존 보고서 값과 나머지 파일/mode는 그대로다. 구형 writer가 이관본을 거절하고
  변경하지 않는 것도 확인했다. 실제 기존 프로젝트를 복사하거나 이관하지 않았다.
- 최초 패키지 fixture는 `/private/tmp`에서 상속된 다른 그룹 소유권 때문에 안전하게 거절됐다.
  원본 권한을 고쳐 우회하지 않고 그 fixture를 보존했다. 사용자 소유 임시 디렉터리 아래
  **새 합성 fixture**에서 성공했다. 이는 소유권 복원 지원 범위의 제한이지 숨겨진 예외가 아니다.
- 기존 설치 Skill/주 설정 및 앞선 닫힌 합성 보고서 등을 포함한 보호 지문 **26개 불변**.
  이번 실행은 설치/cache 교체, 사용자 기록 수정, 커밋·push·공개 게시를 하지 않았다.

개발 archive는 코드/Skill 검수 시점의 snapshot이며, 위 최종 실행 장부를 나중에 덧붙인
문서 digest까지 포함한 재패키징이라고 주장하지 않는다.

## 10. 실제 Agent의 분리 승인·적용 인수 (2026-10-14)

로그인된 격리 CLI 프로필에 §9 개발 패키지를 공식 Plugin 관리로 설치했다. 일반 설치본은
교체하지 않았다. 새 합성 format 4 Project에는 닫힌 질문 Report, 열린 Interview, 미확정
작업 파일, Git 메타데이터 모형, 미참조 객체와 실행 권한을 둔다. 실제 사용자 자료는 없다.

- 같은 실제 Agent 세션이 설치된 Skill을 읽고 check만 호출한 뒤 백업 승인을 요청했다.
  승인 전에는 백업 폴더가 없고 원본 전체 지문이 같았다.
- 사람이 백업·복구 검증만 별도로 승인한 뒤 prepare를 한 번 실행했다. 백업 bytes와
  독립 복구본 bytes/mode가 원본과 같고 원본 전체는 불변이었다. Agent는 검수 결과와
  구형 GIL 비호환을 설명하고 적용 승인을 별도로 요청했으며 apply를 호출하지 않았다.
- 사람이 같은 계획의 원본 적용을 새로 승인했다. 최초 비대화형 실행은 Host의
  `approval policy is never`에서 차단됐다. Agent는 자동 재시도 없이 status의
  `not_applied`/`preserved: true`와 기존 context를 확인했다. 이는 제품 이관 실패가
  아니라 Host 추가 승인 미충족이며, 대화상 동의가 Host 승인을 대신하지 않는 사례다.
- 같은 세션을 `workspace-write`/`on-request` 대화형 경로로 재개하고 승인된 apply
  **한 건만 Allow**했다. 세션 전체/항상 허용이나 sandbox 우회는 사용하지 않았다.
  Agent가 apply 성공 후 status와 context로 기존 질문 Report와 열린 Interview를 확인했다.
- 독립 감사에서 state만 format 4→5로 바뀌었고, format/visuals 외 기존 YAML 값이 같았다.
  나머지 전체 경로·파일 bytes·mode, 원본 백업 bytes, 복구본 bytes/mode가 모두 보존됐다.
  별도 새 MCP 서버도 `applied_state_present`/`preserved: true`를 반환했다.
  일반 설치 설정/Skill과 앞선 닫힌 합성 기록의 보호 지문 26개는 불변이었다.

이는 모의 승인만의 시험이 아니라 **두 차례 실제 사람의 답변을 전달받은 실제 Agent**의
승인 경계 인수다. Host 승인 한 건은 사용자가 승인한 정확한 도구 인수를 확인한 상위 검수자가
처리했다. 새 Desktop 화면, Windows 이관, 실제 사용자 Project 이관을 검수한 것은 아니다.
새 Agent 작업이나 이미지 작성은 추가하지 않았고 합성 백업을 삭제하지 않았다.

### 배포 후보 통합 경계

검수된 생성·작성·재사용·읽기 UI·승인 이관을 `visual-authoring` 로컬 통합 archive로
다시 묶는다. 현재는 24개 도구의 **개발 통합 후보**이며 `publishable: false`를 유지한다.
정식 배포를 완료했다고 표현하지 않는다. 이번 달 배포·홍보 목표에 맞춰 남은 작업은
다음 출시 필수 묶음으로 진행하고, 완료된 세부 검수를 이유 없이 반복하지 않는다.

1. M5 호환성: action surface 2/format 4..5/도구 schema/업데이트 판정/배포 manifest 정합.
2. 통합 후보 필수 회귀·보안·라이선스 및 M6 설치/화면 인수. 플랫폼 지원 범위를 사실대로 고정.
3. 비개발자 설치·승인 이관·실패 시 보존 안내, 릴리스 노트와 홍보 초안 준비.
4. PR/CI와 별도 게시 승인 뒤 정식 공개. 홍보에는 실제 지원 범위와 미서명 등 제한을 명시.

월내 목표는 안전 게이트를 생략하는 승인이 아니다. Windows 새 이미지 작성/이관 미검수와
macOS 개발 한도는 숨기지 않으며, 공개 패키지 생성/설치본 교체/게시에는 별도 절차를 따른다.

## 11. M5 호환성 선언·업데이트 판정·배포 manifest 정합 (2026-10-14)

| 대상 | action surface | 저장 범위 | 도구 수 | 이관 표시 |
| --- | --- | --- | --- | --- |
| 기본 macOS 빌드 | 1 | 4 | 19 | none |
| macOS arm64 이미지 작성 후보 | 2 | 4..5 | 24 | requires_review |

protocol/descriptor schema와 Monitor View/NodeDetail schema는 1을 유지한다. 이 표는
이미 설치된 preview.3 파일을 수정하는 것이 아니라 새 빌드·패키지의 계약이다. 다른 플랫폼의
미검수 이미지 지원은 선언하지 않는다. 저장 범위는 완전한 작업 지원 범위이며 기본 빌드의
format 5 읽기 전용 투영을 format 5 쓰기 지원으로 확대 해석하지 않는다.

Core의 실제 challenge 응답과 패키지 profile/receipt를 대조하며, 같은 실행 파일에서
tools/list 전체 schema·visibility, manifest의 상대 경로 native 진입점, UI resource digest,
압축 왕복·실행 비트·외부 라이브러리·고지를 함께 검사한다. manifest에 Host가 모르는 자체
호환성 필드를 추가하지 않고 배포 receipt에 명시한다. 옛 개발 receipt에 이 선언이 없으면
재빌드 대상이며 설치 cache를 수동 수정하지 않는다. 공개 채널의 기존 계약은 그대로다.

이미지 surface 2의 업데이트 기록은 format 4..5와 `requires_review`가 필수다. 같은 버전의
다른 bytes/계약은 거절하고, 구형→신형 및 신형→구형을 자동 안전 업데이트/롤백으로 판정하지
않는다. 기존 정책의 `blocked / explicit_contract_and_migration_review_required`는 설치 거절
오류가 아니라 별도 계약·이관 검토가 필요하다는 유지관리 판정이다. Plugin 설치 승인과
프로젝트별 백업/적용 승인은 계속 별개다. 구형 JS consumer는 surface 2를 거절해야 한다.

M5 시점에는 작성 개발 후보의 공개 staging 차단을 유지했다(후속 M6은 §12). 이 단계를 통과해도 자동 게시·자동
설치·실제 사용자 이관을 승인하지 않는다. 남은 출시 묶음은 공개 채널용 후보 승격 정책과
정확한 지원 범위/버전 결정, 통합 Host 인수, PR/CI·게시 승인이다. **UI 디자인 다듬기는
이 기능·배포 계약을 변경하지 않는 마지막 단계**로 둔다. 월내 배포·홍보를 목표로 하되
기록 보존·보안·고지·실제 화면 인수는 생략하지 않는다.

### M5 실행 결과

- 작성 Core 전체: 1,338 통과 / 0 실패 / 3 ignored. 기본 Core 순차 최종 실행:
  1,321 통과 / 0 실패 / 2 ignored. 실제 preview.3 writer의 format 5 거절·bytes 보존도 1건 통과.
- UI/Plugin/배포/고지 합산 실행: 184 통과 / 0 실패 / 기존 skip 4.
  마지막 업데이트 판정 회귀 추가 후 배포/고지 묶음은 87 통과 / 0 실패로 다시 확인했다.
- macOS release 패키지의 기본 19도구와 작성 24도구 모두 archive 왕복, 실행 비트,
  native fresh probe의 정확한 surface/storage, manifest 진입점, schema/visibility,
  embedded UI와 고지 검사를 통과했다. 둘 다 publishable false이며 설치하지 않았다.
- 최초 기본 Core 실행의 MCP 2건 실패는 같은 target에 겹쳐 돌린 feature 시험 빌드가
  실행 파일을 바꾼 검수 경합이었다. 해당 로그를 보존하고 동시 빌드를 없앤 기본 전체
  재실행에서 통과했다. 원래 실패를 없었던 것으로 세지 않는다.
- 기존 설정/Skill/닫힌 합성 기록 보호 지문 26개 불변. 사용자 기록·설치본·UI 디자인은
  변경하지 않았다. 새 Host 화면, Windows 작성, 원격 CI/게시를 이번 결과로 대신하지 않는다.

## 12. M6 공개 배포 경로 통합 (2026-10-14)

사용자는 기존 설치본/사용자 기록을 보존한 공개 배포 경로 통합과 격리 설치/화면 검수를
승인했다. 게시·기존 설치본 교체·실제 사용자 Project 이관은 승인 범위가 아니다.

- `unsigned-preview prepare`의 기본 profile은 그대로다. `--visual-authoring`을 명시하고
  입력의 feature/compatibility/migration이 정확히 일치할 때만 작성 후보를 준비한다.
- clean source commit 요구를 유지한다. 미커밋 source를 복사하거나 receipt의 dirty를
  바꿔 공개 후보로 만들지 않는다. 현재 개발 tree의 설치 인수와 clean CI 후보 인수는 구분한다.
- 기존 default receipt schema 2는 유지한다. 작성 후보는 schema 3으로 runtime_profile,
  compatibility, migration을 필수 선언한다. schema 2로 새 계약을 은폐하지 않는다.
- 경고문에 새 Project format 5, 기존 Project 자동 변환 없음, 별도 백업/적용 승인,
  Plugin downgrade로 기록 형식이 돌아가지 않음을 포함한다. 미서명/공증 없음 고지는 유지한다.
- 압축을 다시 푼 동일 native 실행 파일의 surface 2/format 4..5/24 tools/embedded UI를
  검증하고 update-record에도 requires_review를 남긴다. 자동 업데이트 판정을 완화하지 않는다.
- CI의 명시적 runtime_profile 선택으로 build와 staging을 함께 선택한다. 기본 PR은
  기존 기본 빌드이며 이미지 CI 후보는 수동 선택한다. contents: read, 고정 action SHA,
  게시 기능 없음, 임의 입력의 직접 shell 삽입 금지 경계를 유지한다.

공개 전 승인/증거: 검수된 source 커밋·PR·push/CI, 불변 배포 버전/지원 범위 결정,
같은 clean CI 산출물의 설치/화면 인수, marketplace/릴리스 게시 승인. 정식 stable 표시는
현재 preview 전용 제작 정책을 조용히 바꾸지 않고 별도 확정한다. Windows 작성과 Claude
배포 인수는 이 macOS 후보 성공으로 대신하지 않는다. 마지막 UI 마감 이후 payload가 바뀌면
새 digest로 재패키징한다.

### M6 이번 실행과 남은 게이트

- 배포/고지 89건 통과. 첫 실행은 기존 staging 거절문과 CI 명령문 모양을 고정한 시험이
  변경된 정책/전체 glob 실행과 맞지 않아 실패했다. 각각 명시적 profile 거절, 전체 시험 포함,
  output parent 선행 조건으로 보정했다. 실제 입력 무결성/clean source 차단 시험은 유지했다.
- release 개발 archive 왕복과 native 24도구/descriptor/UI 검수 통과. 일반 설치와 별개인
  새 임시 Codex 프로필에 공식 CLI marketplace add/plugin add로 설치했다. 설치된 모든 payload
  bytes/mode가 후보와 일치하고 actual Core probe가 surface 2/format 4..5를 반환했다.
- **그 설치본 실행 파일**로 합성 C2의 보고서·이미지 bytes를 읽고 새 서버로 재실행했다.
  report digest/이미지 digest/참조/설명/출처/scope가 유지되고 원본 state bytes가 같았다.
  harness 최초 owner 문자열은 typed schema가 거절했고 object로 수정했다. 다음 비교는
  서버 epoch가 포함된 revision까지 같다고 잘못 가정해 실패했으며, process identity와
  영속 보고서·이미지의 비교를 분리한 뒤 통과했다. 제품 기록은 수정하지 않았다.
- 실제 dirty 후보의 공개 staging은 output 생성 전에 거절됐다. 버전 검사용 합성 값은
  공개 버전 예약/게시가 아니다. 소스를 가짜 clean snapshot으로 만들지 않았다.
- 기존 임시 MCP 서버 UI와 후보 embedded HTML은 SHA-256이 동일하다. 그 기존 연결의
  실제 Codex 카드에서 C2 보고서/이미지/설명/출처 DOM, 원본 확대·버튼 복귀 및 파랑·주황
  PNG의 화면 렌더링을 확인했다. 이미지 클릭 복귀는 자동 제어가 가림으로 거절하여
  표시된 텍스트 복귀 버튼을 사용했다. 채팅 병행의 이번 사용자 확인은 대기다.
  이는 **동일 UI 회귀 확인**이지 새 후보 자체의 Desktop Plugin 연결/clean CI 설치 인수는 아니다.
- 기존 설정/설치 Skill/닫힌 합성 보고서 보호 지문 26개 불변. UI 코드·설치본·사용자 기록,
  Git 커밋·push·원격 CI·marketplace·릴리스는 변경/실행하지 않았다. 격리 설치는 유지한다.

다음 승인 묶음은 검수 변경분 커밋·PR·push/CI 실행이다. clean CI 후보 bytes가 정해지면
임시 Desktop 연결 인수와 UI 마감을 거친 후 정확한 버전·지원 범위·게시 위치를 제시해
최종 게시 승인을 받는다. 홍보 게시와 사용자별 실제 이관 승인은 이 승인을 대신하지 않는다.
