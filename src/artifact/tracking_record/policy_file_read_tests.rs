//! 루트 `.gilignore` 읽기 전용 관측의 시험 — **합성 임시 폴더**만 쓴다.
//!
//! 폴더는 프로세스 번호가 붙은 새 임시 폴더이고, `.gil/state.yaml`·`.gitignore` 같은 이름도 시험이 만든 바이트다.
//! 실제 사용자 프로젝트·`gil start`·실제 승인 기록을 쓰지 않는다. 관측 도중의 교체·삭제·변경은 [`observe`] 의 틈
//! ([`Pause`])에서 정확히 한 번 일으킨다 — 잠이나 스레드 경쟁에 맡기지 않는다.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::policy_file_read::{Pause, ReadProblem, observe, read_root_policy_file};
use super::sync::{DiskReading, PolicyFileReading, SyncBase, classify_policy_file};
use crate::artifact::{ContentDigest, digest_of};

// ── 도구 ───────────────────────────────────────────────────────────────────

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gil-pfr-{}-{label}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("시험이 쓸 자리를 만들 수 있어야 한다");
    dir
}

fn address(bytes: &[u8]) -> ContentDigest {
    digest_of(bytes).expect("메모리의 바이트는 언제나 읽힌다")
}

fn policy(root: &Path) -> PathBuf {
    root.join(".gilignore")
}

/// 틈 없이 한 번 관측한다.
fn quiet(root: &Path) -> Result<Option<ContentDigest>, ReadProblem> {
    observe(root, &mut |_| {})
}

/// 정해진 틈에서 정확히 한 번 `act` 를 일으킨다. 그 틈에 닿지 않았으면 시험이 실패한다.
fn at_pause(root: &Path, when: Pause, act: impl FnOnce()) -> Result<Option<ContentDigest>, ReadProblem> {
    let mut act = Some(act);
    let result = observe(root, &mut |now| {
        if now == when {
            (act.take().expect("같은 틈이 두 번 오지 않는다"))();
        }
    });
    assert!(act.is_none(), "{when:?} 틈에 닿지 않았다: {result:?}");
    result
}

fn changed(result: &Result<Option<ContentDigest>, ReadProblem>) -> bool {
    matches!(result, Err(ReadProblem::Changed { .. }))
}

/// 폴더 아래 모든 항목의 종류·내용(연결은 대상 글자, 특수 항목은 열지 않음)·수정 시각 — 「아무것도 바꾸지 않았다」를
/// 재는 눈. **접근 시각은 재지 않는다** — 읽기가 그것을 갱신할 수 있다.
#[derive(Debug, PartialEq, Eq)]
enum Seen {
    Dir(Option<SystemTime>),
    File(Vec<u8>, Option<SystemTime>),
    Link(PathBuf),
    Other(Option<SystemTime>),
}

fn tree(root: &Path) -> BTreeMap<PathBuf, Seen> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Seen>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            let modified = meta.modified().ok();
            let key = path.strip_prefix(root).unwrap().to_path_buf();
            let kind = meta.file_type();
            if kind.is_symlink() {
                out.insert(key, Seen::Link(fs::read_link(&path).unwrap()));
            } else if kind.is_dir() {
                out.insert(key, Seen::Dir(modified));
                walk(root, &path, out);
            } else if kind.is_file() {
                out.insert(key, Seen::File(fs::read(&path).unwrap(), modified));
            } else {
                out.insert(key, Seen::Other(modified));
            }
        }
    }
    let mut out = BTreeMap::new();
    out.insert(PathBuf::new(), Seen::Dir(fs::symlink_metadata(root).unwrap().modified().ok()));
    walk(root, root, &mut out);
    out
}

// ── 일반 파일: 실제 바이트 그대로 ─────────────────────────────────────────

#[test]
fn a_regular_file_is_observed_as_its_exact_bytes() {
    let cases: [&[u8]; 8] = [
        b"target/\n",
        b"target/\r\n",
        b"target/",
        b"\xEF\xBB\xBFtarget/\n",
        b"  # comment  \ntarget/\n",
        b"target/\n\n",
        b"\xFF\xFE\x00not utf-8\n",
        b"",
    ];
    let mut seen = Vec::new();
    for (at, bytes) in cases.iter().enumerate() {
        let root = scratch(&format!("bytes-{at}"));
        fs::write(policy(&root), bytes).unwrap();
        let reading = read_root_policy_file(&root);
        assert_eq!(reading, DiskReading::Present(address(bytes)), "{bytes:?}");
        seen.push(reading);
        let _ = fs::remove_dir_all(&root);
    }
    // 줄바꿈·BOM·공백·주석·인코딩을 정규화하지 않으므로 모두 다른 관측이다. 빈 파일도 있는 파일이다.
    for (a, left) in seen.iter().enumerate() {
        for right in &seen[a + 1..] {
            assert_ne!(left, right);
        }
    }
    assert_eq!(seen.last(), Some(&DiskReading::Present(address(b""))));
}

#[test]
fn the_public_reading_is_the_quiet_observation() {
    let root = scratch("public");
    assert_eq!(quiet(&root), Ok(None));
    assert_eq!(read_root_policy_file(&root), DiskReading::Absent);
    fs::write(policy(&root), b"dist/\n").unwrap();
    assert_eq!(quiet(&root), Ok(Some(address(b"dist/\n"))));
    assert_eq!(read_root_policy_file(&root), DiskReading::Present(address(b"dist/\n")));
    fs::remove_file(policy(&root)).unwrap();
    fs::create_dir(policy(&root)).unwrap();
    let DiskReading::Unreadable { reason } = read_root_policy_file(&root) else { panic!("디렉터리를 읽었다") };
    assert_eq!(reason, quiet(&root).unwrap_err().to_string());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_steps_run_in_the_documented_order() {
    let root = scratch("order");
    let mut steps = Vec::new();
    assert_eq!(observe(&root, &mut |step| steps.push(step)), Ok(None));
    assert_eq!(steps, [Pause::RootOpened, Pause::BeforeRootRecheck]);

    fs::write(policy(&root), b"x\n").unwrap();
    let mut steps = Vec::new();
    assert!(observe(&root, &mut |step| steps.push(step)).unwrap().is_some());
    assert_eq!(
        steps,
        [
            Pause::RootOpened,
            Pause::EntrySeen,
            Pause::FileOpened,
            Pause::AfterRead,
            Pause::BeforeNameRecheck,
            Pause::BeforeRootRecheck
        ]
    );
    let _ = fs::remove_dir_all(&root);
}

// ── 없음은 확인된 루트에서만 ──────────────────────────────────────────────

#[test]
fn absent_means_a_confirmed_root_without_the_policy_file() {
    let root = scratch("absent");
    // 다른 이름의 정책 비슷한 파일은 대신 읽지 않는다.
    fs::write(root.join(".gitignore"), b"target/\n").unwrap();
    fs::create_dir(root.join("sub")).unwrap();
    fs::write(root.join("sub").join(".gilignore"), b"*\n").unwrap();
    fs::write(root.join("work.txt"), b"user file").unwrap();
    assert_eq!(read_root_policy_file(&root), DiskReading::Absent);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_missing_or_unconfirmed_root_is_never_absent() {
    let base = scratch("roots");
    let missing = base.join("no-such-root");
    let missing_parent = base.join("no-parent").join("root");
    let file_root = base.join("a-file");
    fs::write(&file_root, b"not a folder").unwrap();
    for root in [&missing, &missing_parent, &file_root] {
        assert!(matches!(quiet(root), Err(ReadProblem::RootUnavailable { .. })), "{root:?}: {:?}", quiet(root));
        assert!(matches!(read_root_policy_file(root), DiskReading::Unreadable { .. }));
    }
    // 상대 경로는 현재 작업 폴더로 풀지 않는다(작업 폴더를 바꾸지 않고 확인한다).
    for relative in ["relative-root", ".", ""] {
        assert_eq!(quiet(Path::new(relative)), Err(ReadProblem::RootNotAbsolute), "{relative:?}");
    }
    assert!(!missing.exists(), "관측이 루트를 만들었다");
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn a_directory_at_the_policy_name_is_unreadable_not_absent() {
    let root = scratch("dir");
    fs::create_dir(policy(&root)).unwrap();
    fs::write(policy(&root).join("inside"), b"x").unwrap();
    assert_eq!(quiet(&root), Err(ReadProblem::NotRegular { kind: "디렉터리" }));
    let _ = fs::remove_dir_all(&root);
}

// ── 관측 중 바뀜 (모든 운영체제에서 같은 핸들 쓰기로 일으킬 수 있는 것) ───────────────

#[test]
fn a_same_length_rewrite_right_after_reading_is_refused() {
    // 비교 대상 메타데이터가 달라진 같은 길이 변경은 거절한다. 읽기 전 메타데이터를 잰 뒤(열고 잰 뒤, 또는 한 번 흘려
    // 읽은 뒤) 같은 크기로 다시 쓰고, 수정 시각을 원래 값과 충분히 다른 고정 값으로 명시해 그 설정이 실제로 반영됐음을
    // 확인한다. 같은 길이의 쓰기가 언제나 메타데이터 차이로 드러난다는 계약은 아니다 — 읽기 후 메타데이터 비교가
    // 달라진 값을 발견할 때 지문을 내놓지 않는다는 것만 고정한다.
    let distinct = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
    for when in [Pause::FileOpened, Pause::AfterRead] {
        let root = scratch("rewrite");
        fs::write(policy(&root), b"aaaa\n").unwrap();
        let result = at_pause(&root, when, || {
            let original = fs::metadata(policy(&root)).unwrap().modified().unwrap();
            assert_ne!(original, distinct, "준비: 원래 수정 시각이 지정할 값과 같다");
            fs::write(policy(&root), b"bbbb\n").unwrap();
            fs::File::options().write(true).open(policy(&root)).unwrap().set_modified(distinct).unwrap();
            let now = fs::metadata(policy(&root)).unwrap().modified().unwrap();
            assert_eq!(now, distinct, "준비: 이 운영체제에서 수정 시각 설정이 반영되지 않았다");
        });
        assert!(changed(&result), "{when:?}: {result:?}");
        // 관측이 실패해도 파일을 되돌리거나 지우지 않는다.
        assert_eq!(fs::read(policy(&root)).unwrap(), b"bbbb\n");
        let _ = fs::remove_dir_all(&root);
    }
}

#[test]
fn growth_after_opening_is_refused() {
    let root = scratch("grow");
    fs::write(policy(&root), b"a\n").unwrap();
    let result = at_pause(&root, Pause::FileOpened, || fs::write(policy(&root), b"a\nb\n").unwrap());
    assert!(changed(&result), "{result:?}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_change_between_the_reads_and_the_name_recheck_is_not_caught_by_content_alone() {
    // 한 번 흘려 읽기와 그 전·후 메타데이터 비교(내용에 관한 마지막 검사)를 지난 뒤 바뀐 내용은 탐지 범위 밖이다.
    // 이름·루트 재확인은 정체성만 견주고, 같은 파일이 같은 이름에 있으므로 Present(앞서 읽은 바이트의 지문)다 —
    // 반환 시점의 디스크 내용을 보증하지 않는다는 문서화된 한계를 고정한다.
    let root = scratch("late");
    fs::write(policy(&root), b"first\n").unwrap();
    let result = at_pause(&root, Pause::BeforeNameRecheck, || fs::write(policy(&root), b"later\n").unwrap());
    assert_eq!(result, Ok(Some(address(b"first\n"))));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_change_reverted_between_observations_is_not_detected() {
    // 관측 사이에 바뀌었다가 같은 바이트로 되돌아온 변경은 구별하지 못한다 — 그렇게 주장하지 않는다.
    let root = scratch("aba");
    fs::write(policy(&root), b"same\n").unwrap();
    let first = read_root_policy_file(&root);
    fs::write(policy(&root), b"other\n").unwrap();
    fs::write(policy(&root), b"same\n").unwrap();
    assert_eq!(read_root_policy_file(&root), first);
    let _ = fs::remove_dir_all(&root);
}

// ── 읽기 전용 경계 ────────────────────────────────────────────────────────

#[test]
fn observing_writes_nothing_and_creates_no_entry() {
    let root = scratch("read-only");
    fs::create_dir_all(root.join(".gil").join("objects")).unwrap();
    fs::write(root.join(".gil").join("state.yaml"), b"format: 9\n# synthetic\n").unwrap();
    fs::write(root.join(".gitignore"), b"target/\n").unwrap();
    fs::write(root.join("work.txt"), b"user file").unwrap();
    for content in [None, Some(&b"node_modules/\n"[..]), Some(&b""[..])] {
        match content {
            Some(bytes) => fs::write(policy(&root), bytes).unwrap(),
            None => {
                let _ = fs::remove_file(policy(&root));
            }
        }
        let before = tree(&root);
        for _ in 0..3 {
            let reading = read_root_policy_file(&root);
            match content {
                Some(bytes) => assert_eq!(reading, DiskReading::Present(address(bytes))),
                None => assert_eq!(reading, DiskReading::Absent),
            }
        }
        assert_eq!(tree(&root), before, "관측이 폴더의 항목·내용·수정 시각을 바꿨다");
    }
    // 실패한 관측도 같다.
    fs::remove_file(policy(&root)).unwrap();
    fs::create_dir(policy(&root)).unwrap();
    let before = tree(&root);
    assert!(matches!(read_root_policy_file(&root), DiskReading::Unreadable { .. }));
    assert_eq!(tree(&root), before);
    let _ = fs::remove_dir_all(&root);
}

// ── 기존 판정표와의 합성 ──────────────────────────────────────────────────

#[test]
fn observations_feed_the_existing_table_without_becoming_approval_or_authorship() {
    let applied = address(b"applied/\n");
    let old = address(b"old/\n");
    let bases = [SyncBase::Present(old.clone()), SyncBase::Absent, SyncBase::Unconfirmed, SyncBase::Present(applied.clone())];
    let root = scratch("classify");
    type R = PolicyFileReading;

    // 판정은 3a 의 표 그대로다 — 관측이 덧붙이는 의미가 없다.
    let mut cases: Vec<(Option<&[u8]>, [R; 4])> = vec![
        (Some(&b"applied/\n"[..]), [R::MatchesApplied, R::MatchesApplied, R::MatchesApplied, R::MatchesApplied]),
        (
            Some(b"old/\n"),
            [R::StaleMatchesBase, R::UserChanged { deleted: false }, R::BaseUnconfirmed, R::UserChanged { deleted: false }],
        ),
        (
            Some(b"edited/\n"),
            [R::UserChanged { deleted: false }, R::UserChanged { deleted: false }, R::BaseUnconfirmed, R::UserChanged { deleted: false }],
        ),
        (None, [R::UserChanged { deleted: true }, R::StaleMatchesBase, R::BaseUnconfirmed, R::UserChanged { deleted: true }]),
    ];
    for (content, expected) in cases.drain(..) {
        match content {
            Some(bytes) => fs::write(policy(&root), bytes).unwrap(),
            None => {
                let _ = fs::remove_file(policy(&root));
            }
        }
        let disk = read_root_policy_file(&root);
        for (base, want) in bases.iter().zip(expected) {
            let kept = base.clone();
            // 같은 관측을 직접 만든 값과 똑같이 판정한다 — 관측 경로가 다른 판정을 내지 않는다.
            let direct = match content {
                Some(bytes) => DiskReading::Present(address(bytes)),
                None => DiskReading::Absent,
            };
            assert_eq!(classify_policy_file(&applied, base, &disk), classify_policy_file(&applied, base, &direct));
            assert_eq!(classify_policy_file(&applied, base, &disk), want, "{content:?} / {base:?}");
            assert_eq!(base, &kept, "판정이 기준을 바꿨다");
        }
    }
    // `UserChanged` 는 「적용 원문과도 기준과도 다르다」의 내부 이름일 뿐이다. 관측 결과에는 작성자를 가리킬 칸이 없고,
    // Core 가 직접 쓴 바이트와 사용자가 쓴 바이트는 같은 관측이 된다.
    fs::write(policy(&root), b"edited/\n").unwrap();
    let by_one = read_root_policy_file(&root);
    fs::remove_file(policy(&root)).unwrap();
    fs::write(policy(&root), b"edited/\n").unwrap();
    assert_eq!(read_root_policy_file(&root), by_one);

    // 읽지 못함은 어떤 기준에서도 동기화됨·구판 잔류가 되지 않는다.
    fs::remove_file(policy(&root)).unwrap();
    fs::create_dir(policy(&root)).unwrap();
    let disk = read_root_policy_file(&root);
    for base in &bases {
        assert!(matches!(classify_policy_file(&applied, base, &disk), R::Unreadable { .. }), "{base:?}");
    }
    let _ = fs::remove_dir_all(&root);
}

// ── Unix: 연결 파일·특수 항목·권한·이름과 루트의 교체 ───────────────────────

#[cfg(unix)]
mod unix {
    use std::ffi::CString;
    use std::fs;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{PermissionsExt, symlink};

    use super::*;

    fn mkfifo(at: &Path) {
        let name = CString::new(at.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o644) }, 0, "{}", std::io::Error::last_os_error());
    }

    fn running_as_root() -> bool {
        unsafe { libc::geteuid() == 0 }
    }

    #[test]
    fn links_are_never_followed_and_a_dangling_link_is_not_absent() {
        let root = scratch("links");
        let outside = scratch("links-outside");
        fs::write(outside.join("rules"), b"secret/\n").unwrap();

        symlink(outside.join("rules"), policy(&root)).unwrap();
        assert_eq!(quiet(&root), Err(ReadProblem::NotRegular { kind: "연결 파일" }));
        fs::remove_file(policy(&root)).unwrap();

        symlink(outside.join("gone"), policy(&root)).unwrap();
        assert_eq!(quiet(&root), Err(ReadProblem::NotRegular { kind: "연결 파일" }), "끊긴 연결이 없음이 됐다");
        fs::remove_file(policy(&root)).unwrap();

        symlink(&outside, policy(&root)).unwrap();
        assert_eq!(quiet(&root), Err(ReadProblem::NotRegular { kind: "연결 파일" }));

        assert_eq!(fs::read(outside.join("rules")).unwrap(), b"secret/\n");
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn a_root_that_is_itself_a_link_is_not_opened() {
        let base = scratch("root-link");
        let real = base.join("real");
        fs::create_dir(&real).unwrap();
        fs::write(real.join(".gilignore"), b"x\n").unwrap();
        let link = base.join("link");
        symlink(&real, &link).unwrap();
        assert!(matches!(quiet(&link), Err(ReadProblem::RootUnavailable { .. })), "{:?}", quiet(&link));
        // 끝의 `/` 도 연결을 풀게 만들지 못한다.
        let slashed = PathBuf::from(format!("{}/", link.display()));
        assert!(matches!(quiet(&slashed), Err(ReadProblem::RootUnavailable { .. })), "{:?}", quiet(&slashed));
        let dangling = base.join("dangling");
        symlink(base.join("gone"), &dangling).unwrap();
        assert!(matches!(quiet(&dangling), Err(ReadProblem::RootUnavailable { .. })));
        assert_eq!(quiet(&real), Ok(Some(address(b"x\n"))));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn special_entries_are_refused_without_opening_or_waiting() {
        let root = scratch("fifo");
        mkfifo(&policy(&root));
        // 쓰는 쪽이 없는 FIFO 를 열었다면 이 시험이 멈춘다. 종류를 보고 열지 않는다.
        assert_eq!(quiet(&root), Err(ReadProblem::NotRegular { kind: "FIFO" }));
        fs::remove_file(policy(&root)).unwrap();

        let listener = std::os::unix::net::UnixListener::bind(policy(&root)).expect("짧은 임시 경로에 소켓을 만들 수 있어야 한다");
        assert_eq!(quiet(&root), Err(ReadProblem::NotRegular { kind: "소켓" }));
        drop(listener);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_hard_linked_regular_file_is_read_like_any_regular_file() {
        // 정책 엔진 트리 평가처럼 링크 수로 거절하지 않는다 — 기존 경로 계약보다 넓은 거절을 만들지 않았다.
        let root = scratch("hard");
        fs::write(root.join("other-name"), b"a/\n").unwrap();
        fs::hard_link(root.join("other-name"), policy(&root)).unwrap();
        assert_eq!(quiet(&root), Ok(Some(address(b"a/\n"))));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn permission_failures_are_unreadable_not_absent() {
        if running_as_root() {
            eprintln!("root 는 권한을 무시하므로 접근 실패를 만들 수 없다 — 시험을 건너뛴다");
            return;
        }
        let root = scratch("perm-file");
        fs::write(policy(&root), b"x\n").unwrap();
        fs::set_permissions(policy(&root), fs::Permissions::from_mode(0o000)).unwrap();
        assert!(matches!(quiet(&root), Err(ReadProblem::EntryUnavailable { .. })), "{:?}", quiet(&root));
        fs::set_permissions(policy(&root), fs::Permissions::from_mode(0o644)).unwrap();
        let _ = fs::remove_dir_all(&root);

        // 목록을 읽을 수 없는 루트 — 그 안에 정책 파일이 있는지 모른다.
        let root = scratch("perm-root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();
        assert!(matches!(quiet(&root), Err(ReadProblem::RootUnavailable { .. })), "{:?}", quiet(&root));
        // 들어갈 수는 있지만 항목 종류를 볼 수 없는 루트(읽기만 있고 탐색 권한 없음).
        fs::set_permissions(&root, fs::Permissions::from_mode(0o400)).unwrap();
        let result = quiet(&root);
        assert!(!matches!(result, Ok(None)), "확인하지 못한 루트에서 없음이라고 했다: {result:?}");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_entry_replaced_after_its_kind_was_seen_is_refused() {
        // 종류를 본 뒤 열기 전의 교체는 Unix 만 본 (장치, inode)와 견줘 잡는다. Windows 는 이 구간의 일반 파일 교체를
        // 견주지 않으므로(모듈 설명) 이 시험은 Unix 전용이다.
        let outside = scratch("swap-outside");
        fs::write(outside.join("rules"), b"outside/\n").unwrap();
        type Swap = fn(&Path, &Path);
        let swaps: [(&str, Swap); 4] = [
            ("삭제", |p, _| fs::remove_file(p).unwrap()),
            ("연결 파일", |p, o| {
                fs::remove_file(p).unwrap();
                symlink(o.join("rules"), p).unwrap();
            }),
            ("FIFO", |p, _| {
                fs::remove_file(p).unwrap();
                mkfifo(p);
            }),
            ("다른 일반 파일", |p, _| {
                let fresh = p.with_file_name("fresh");
                fs::write(&fresh, b"a/\n").unwrap();
                fs::rename(&fresh, p).unwrap();
            }),
        ];
        for (label, swap) in swaps {
            let root = scratch("swap");
            fs::write(policy(&root), b"a/\n").unwrap();
            let target = policy(&root);
            let result = at_pause(&root, Pause::EntrySeen, || swap(&target, &outside));
            assert!(changed(&result), "{label}: {result:?}");
            let _ = fs::remove_dir_all(&root);
        }
        assert_eq!(fs::read(outside.join("rules")).unwrap(), b"outside/\n");
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn deletion_or_replacement_while_reading_is_refused() {
        // 열어 둔 핸들은 지워진 파일의 바이트를 계속 읽을 수 있다 — 그래서 메타데이터와 이름을 다시 본다.
        for (label, when) in [("삭제(읽는 중)", Pause::FileOpened), ("삭제(이름 재확인 전)", Pause::BeforeNameRecheck)] {
            let root = scratch("unlink");
            fs::write(policy(&root), b"a/\n").unwrap();
            let target = policy(&root);
            let result = at_pause(&root, when, || fs::remove_file(&target).unwrap());
            assert!(changed(&result), "{label}: {result:?}");
            let _ = fs::remove_dir_all(&root);
        }
        // 같은 바이트의 새 파일로 이름을 바꿔 끼워도 다른 파일이다.
        let root = scratch("replace");
        fs::write(policy(&root), b"a/\n").unwrap();
        let target = policy(&root);
        let result = at_pause(&root, Pause::BeforeNameRecheck, || {
            let fresh = target.with_file_name("fresh");
            fs::write(&fresh, b"a/\n").unwrap();
            fs::rename(&fresh, &target).unwrap();
        });
        assert!(changed(&result), "{result:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_root_replaced_or_removed_during_observation_is_refused() {
        // 없음 후보: 처음 연 루트가 지워졌다.
        let root = scratch("root-gone");
        let result = at_pause(&root, Pause::RootOpened, || fs::remove_dir(&root).unwrap());
        assert!(changed(&result), "지워진 루트에서 없음이라고 했다: {result:?}");

        // 없음 후보: 루트를 옮기고 같은 경로에 빈 폴더를 놓았다.
        let root = scratch("root-swap-empty");
        let moved = root.with_file_name(format!("{}-moved", root.file_name().unwrap().to_string_lossy()));
        let _ = fs::remove_dir_all(&moved);
        let result = at_pause(&root, Pause::BeforeRootRecheck, || {
            fs::rename(&root, &moved).unwrap();
            fs::create_dir(&root).unwrap();
        });
        assert!(changed(&result), "{result:?}");
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&moved);

        // 있음: 루트를 옮기고 같은 바이트의 정책 파일이 든 새 폴더를 놓았다 — 내용이 같아도 다른 루트다.
        let root = scratch("root-swap-same");
        fs::write(policy(&root), b"a/\n").unwrap();
        let moved = root.with_file_name(format!("{}-moved", root.file_name().unwrap().to_string_lossy()));
        let _ = fs::remove_dir_all(&moved);
        let result = at_pause(&root, Pause::BeforeRootRecheck, || {
            fs::rename(&root, &moved).unwrap();
            fs::create_dir(&root).unwrap();
            fs::write(policy(&root), b"a/\n").unwrap();
        });
        assert!(changed(&result), "{result:?}");
        // 실패한 관측은 옮겨진 원래 루트도 건드리지 않는다.
        assert_eq!(fs::read(policy(&moved)).unwrap(), b"a/\n");
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&moved);
    }
}
