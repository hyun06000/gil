//! 독립 추적 정책 엔진의 시험 — **합성 프로젝트**만 쓴다(실제 사용자 자료·`gil start` 없음).
//!
//! 각 시험은 프로세스 번호가 붙은 임시 폴더를 새로 만든다. 고정 경로를 다른 빌드 구성과 나눠 쓰다
//! 앞선 실행의 흔적을 읽는 일을 막는다.

use std::fs;
use std::path::{Path, PathBuf};

use super::walk::listed;
use super::*;
use crate::artifact::{IgnoreSource, ObjectStore, PolicyVersion, TrackingPolicy};

// ── 도구 ───────────────────────────────────────────────────────────────────

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gil-gilignore-{}-{label}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("시험이 쓸 자리를 만들 수 있어야 한다");
    dir
}

fn write(root: &Path, path: &str, bytes: &[u8]) {
    let at = root.join(path);
    if let Some(parent) = at.parent() {
        fs::create_dir_all(parent).expect("부모 디렉터리를 만들 수 있어야 한다");
    }
    fs::write(&at, bytes).expect("파일을 쓸 수 있어야 한다");
}

fn policy(text: &str) -> ApprovedPolicy {
    ApprovedPolicy::compile(EngineVersion::V1, text.as_bytes())
        .unwrap_or_else(|err| panic!("정책을 세우지 못했다: {err}"))
}

fn rejected(text: &[u8]) -> RulesRejected {
    match ApprovedPolicy::compile(EngineVersion::V1, text) {
        Ok(_) => panic!("해석할 수 없는 원문으로 정책을 세웠다: {:?}", String::from_utf8_lossy(text)),
        Err(err) => err,
    }
}

fn evaluate(root: &Path, policy: &ApprovedPolicy) -> TreeEvaluation {
    evaluate_tree(root, policy).unwrap_or_else(|err| panic!("평가가 거절됐다: {err}"))
}

fn refused(root: &Path, policy: &ApprovedPolicy) -> EvaluationError {
    match evaluate_tree(root, policy) {
        Ok(result) => panic!("거절해야 할 트리를 받아들였다: {result:?}"),
        Err(err) => err,
    }
}

fn tracked(result: &TreeEvaluation) -> Vec<String> {
    result.tracked().iter().map(|file| file.path.to_string()).collect()
}

fn excluded(result: &TreeEvaluation) -> Vec<String> {
    result.excluded().iter().map(|entry| entry.path.to_string()).collect()
}

fn entry<'a>(result: &'a TreeEvaluation, path: &str) -> &'a ExcludedEntry {
    result
        .excluded()
        .iter()
        .find(|entry| entry.path.as_str() == path)
        .unwrap_or_else(|| panic!("{path} 이(가) 제외 목록에 없다: {:?}", excluded(result)))
}

fn decide(policy: &ApprovedPolicy, path: &str, kind: PathKind) -> Decision {
    policy.decide(&EntryPath::parse(path).expect("시험 경로는 계약을 지킨다"), kind)
}

fn is_excluded(policy: &ApprovedPolicy, path: &str) -> bool {
    matches!(decide(policy, path, PathKind::File), Decision::Excluded { .. })
}

/// 목록을 읽은 디렉터리 — 두 번 훑으므로 같은 자리가 두 번씩 나온다. 중복을 지워 돌려준다.
fn listed_dirs() -> Vec<String> {
    let mut dirs = listed::take();
    dirs.sort();
    dirs.dedup();
    dirs
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

// ── 정책 정체성 ────────────────────────────────────────────────────────────

#[test]
fn the_same_engine_and_bytes_give_the_same_identity_and_evaluation() {
    let text = "node_modules/\ndata/\n!data/keep.csv\n";
    let first = policy(text);
    let second = policy(text);
    assert_eq!(first.identity(), second.identity());
    let id = first.identity().id();
    assert!(id.starts_with("gil-gilignore/1:sha256:"), "{id}");
    assert_eq!(id.len(), "gil-gilignore/1:sha256:".len() + 64);
    assert_eq!(first.identity().engine().number(), 1);
    assert_eq!(EngineVersion::from_number(1), Some(EngineVersion::V1));
    assert_eq!(EngineVersion::from_number(2), None, "모르는 엔진 번호는 받지 않는다");

    // 같은 내용을 서로 다른 절대 위치에 두어도 평가가 같다 — 위치는 정체성에도 결과에도 없다.
    let build = |label: &str| {
        let root = scratch(label);
        write(&root, "src/main.rs", b"fn main() {}");
        write(&root, "node_modules/pkg/index.js", b"x");
        write(&root, "data/keep.csv", b"k");
        write(&root, "data/drop.csv", b"d");
        root
    };
    let one = evaluate(&build("identity-a"), &first);
    let two = evaluate(&build("identity-b"), &second);
    assert_eq!(one, two);
    assert_eq!(tracked(&one), strings(&["data/keep.csv", "src/main.rs"]));
}

#[test]
fn any_byte_change_is_a_different_identity_and_the_digest_is_the_object_address() {
    let base = policy("node_modules/\n");
    for variant in ["node_modules/\r\n", "node_modules/", "# 의존성\nnode_modules/\n", "node_modules/ \n"] {
        assert_ne!(
            base.identity(),
            policy(variant).identity(),
            "원문 바이트가 다르면 해석이 같아도 다른 정체성이다: {variant:?}"
        );
    }

    // 계약 §5.1: rules.digest 는 승인 원문 객체의 주소다 — 같은 바이트를 창고에 넣은 주소와 같다.
    let root = scratch("identity-object");
    let store = ObjectStore::at(&root.join(".gil"));
    let address = store
        .put_blob(&b"node_modules/\n"[..])
        .expect("합성 창고에 원문을 넣을 수 있어야 한다");
    assert_eq!(base.identity().rules_digest(), &address);
}

#[test]
fn the_legacy_gitignore_engine_keeps_its_own_identity_namespace() {
    let text = "node_modules/\n";
    let legacy = TrackingPolicy::from_sources(PolicyVersion::V1, vec![IgnoreSource::at_root(text)])
        .expect("옛 V1 은 그대로 해석한다");
    assert!(legacy.policy_id().starts_with("gil-tracking-policy/1:sha256:"));
    assert_ne!(legacy.policy_id(), policy(text).identity().id());
}

#[test]
fn editing_or_deleting_gitignore_and_git_excludes_do_not_change_the_policy() {
    let root = scratch("gitignore-independent");
    write(&root, ".gitignore", b"data.csv\nsrc/\n");
    write(&root, ".git/info/exclude", b"*\n");
    write(&root, "data.csv", b"1,2");
    write(&root, "src/main.rs", b"x");
    write(&root, "node_modules/pkg/index.js", b"x");
    let applied = policy("node_modules/\n");

    let before = evaluate(&root, &applied);
    // Git 이 무시하는 data.csv·src/ 도, `.git/info/exclude` 의 `*` 도 적용 정책 입력이 아니다.
    assert_eq!(tracked(&before), strings(&[".gitignore", "data.csv", "src/main.rs"]));

    write(&root, ".gitignore", b"*\n");
    let edited = evaluate(&root, &applied);
    assert_eq!(edited.policy(), before.policy());
    assert_eq!(edited, before, ".gitignore 내용 변경은 추적 목록을 바꾸지 않는다");

    fs::remove_file(root.join(".gitignore")).unwrap();
    let deleted = evaluate(&root, &applied);
    assert_eq!(deleted.policy(), before.policy());
    // 남는 차이는 .gitignore 자신이 추적 중인 일반 파일이었다는 것뿐이다.
    assert_eq!(tracked(&deleted), strings(&["data.csv", "src/main.rs"]));
}

// ── 시스템 보호 ────────────────────────────────────────────────────────────

#[test]
fn system_protection_cannot_be_reincluded() {
    for text in [
        "!.git/config\n",
        "!.git\n",
        "!**/.git/hooks/pre-commit\n",
        "!/.gil/state.yaml\n",
        "!.gil/\n",
        "!sub/.gil/x\n",
        "!/.gilignore\n",
        "!.gilignore\n",
    ] {
        let err = rejected(text.as_bytes());
        assert_eq!(err.problems.len(), 1, "{text:?}");
        assert_eq!(err.problems[0].line, 1);
        assert_eq!(err.problems[0].reason, ProblemReason::ReincludesProtectedOnly, "{text:?}");
    }
    // 보호 영역 밖에도 맞을 수 있는 포함 예외는 받아들이되, 보호 대상은 여전히 보호된다.
    let broad = policy("*\n!**\n!*/config\n");
    for (path, kind) in [
        (".git/config", PathKind::File),
        (".git", PathKind::File),
        ("vendor/lib/.git/HEAD", PathKind::File),
        (".gil/state.yaml", PathKind::File),
        (".gilignore", PathKind::File),
    ] {
        assert!(
            matches!(decide(&broad, path, kind), Decision::Protected { .. }),
            "{path} 은(는) 시스템 보호다"
        );
    }
    // `.gil` 이라는 일반 파일은 보통 경로다(§3.4).
    assert!(policy("!.gil\n").decide(&EntryPath::parse(".gil").unwrap(), PathKind::File).is_tracked());

    let root = scratch("protected-tree");
    write(&root, ".git/HEAD", b"ref");
    write(&root, ".git/objects/ab/cd", b"blob");
    write(&root, ".gil/state.yaml", b"format: 4\n");
    write(&root, ".gilignore", b"anything\n");
    write(&root, "lib/.git", b"gitdir: ../.git/modules/lib\n");
    write(&root, "lib/src.rs", b"x");
    listed::take();
    let result = evaluate(&root, &broad);
    assert_eq!(tracked(&result), strings(&["lib/src.rs"]));
    for (path, protection) in [
        (".git", Protection::GitMetadata),
        (".gil", Protection::InternalStore),
        (".gilignore", Protection::PolicyFile),
        ("lib/.git", Protection::GitMetadata),
    ] {
        assert_eq!(entry(&result, path).reason, ExclusionReason::System(protection));
    }
    assert_eq!(listed_dirs(), strings(&["", "lib"]), ".git·.gil 안은 열지 않는다");
}

#[test]
fn the_root_policy_file_is_never_archived_and_never_read_as_policy() {
    let root = scratch("policy-file");
    // 디스크의 .gilignore 는 node_modules 를 제외하지만, 적용 정책은 호출자가 준 원문이다.
    write(&root, ".gilignore", b"node_modules/\n");
    write(&root, "node_modules/pkg/index.js", b"x");
    write(&root, "src/.gilignore.bak", b"not a policy file");
    let result = evaluate(&root, &policy(""));
    assert_eq!(tracked(&result), strings(&["node_modules/pkg/index.js", "src/.gilignore.bak"]));
    let file = entry(&result, ".gilignore");
    assert_eq!(file.reason, ExclusionReason::System(Protection::PolicyFile));
    assert_eq!(file.kind, EntryKind::File);
    assert!(!result.is_blocked());
}

// ── 탐색 생략과 재포함 ─────────────────────────────────────────────────────

#[test]
fn a_fully_excluded_folder_is_not_entered_nor_searched_for_policies() {
    let root = scratch("prune");
    write(&root, "src/main.rs", b"x");
    write(&root, "node_modules/pkg/index.js", b"x");
    write(&root, "node_modules/pkg/.gilignore", b"*\n");
    write(&root, "node_modules/nested/.gil/state.yaml", b"format: 4\n");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/", root.join("node_modules/root-link")).unwrap();
        let fifo = std::ffi::CString::new(
            root.join("node_modules/pipe").as_os_str().as_encoded_bytes(),
        )
        .unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    }
    listed::take();
    let result = evaluate(&root, &policy("node_modules/\n"));
    assert_eq!(tracked(&result), strings(&["src/main.rs"]));
    assert_eq!(excluded(&result), strings(&["node_modules"]));
    assert_eq!(entry(&result, "node_modules").interior, Some(Interior::Skip));
    assert_eq!(
        result.unexplored().map(|e| e.path.to_string()).collect::<Vec<_>>(),
        strings(&["node_modules"])
    );
    assert!(!result.is_blocked(), "들어가지 않은 폴더 안의 하위 정책은 찾지 않는다");
    assert_eq!(listed_dirs(), strings(&["", "src"]), "node_modules 는 열지 않았다");
}

#[test]
fn an_explicit_reinclusion_below_an_excluded_parent_is_found_and_checked() {
    let root = scratch("reinclude");
    write(&root, "data/keep.csv", b"k");
    write(&root, "data/drop.csv", b"d");
    write(&root, "data/sub/a.txt", b"a");
    write(&root, "data/sub/raw/r.bin", b"r");
    write(&root, "data/other/x.txt", b"x");
    let approved = policy("data/\n!data/keep.csv\n!data/sub/\ndata/sub/raw/\n");
    listed::take();
    let result = evaluate(&root, &approved);
    assert_eq!(tracked(&result), strings(&["data/keep.csv", "data/sub/a.txt"]));
    let keep = result.tracked().iter().find(|f| f.path.as_str() == "data/keep.csv").unwrap();
    assert_eq!(keep.by.as_ref().map(|rule| rule.line), Some(2));
    assert_eq!(
        excluded(&result),
        strings(&["data", "data/drop.csv", "data/other", "data/sub/raw"])
    );
    let explored: Vec<_> = result.explored_for_reinclusion().collect();
    assert_eq!(explored.len(), 1);
    assert_eq!(explored[0].0.as_str(), "data");
    assert_eq!(explored[0].1.iter().map(|r| r.line).collect::<Vec<_>>(), vec![2, 3]);
    assert_eq!(entry(&result, "data/other").interior, Some(Interior::Skip));
    assert_eq!(entry(&result, "data/sub/raw").interior, Some(Interior::Skip));
    assert_eq!(listed_dirs(), strings(&["", "data", "data/sub"]));

    // 순서가 뒤집히면 마지막 줄 `data/` 가 이긴다 — 그때는 생략한다.
    listed::take();
    let reversed = evaluate(&root, &policy("!data/keep.csv\ndata/\n"));
    assert!(tracked(&reversed).is_empty());
    assert_eq!(entry(&reversed, "data").interior, Some(Interior::Skip));
    assert_eq!(listed_dirs(), strings(&[""]));

    // 옛 V1(Git 의미)은 바뀌지 않았다 — 제외된 부모 아래는 되살리지 않는다.
    let legacy = TrackingPolicy::from_sources(
        PolicyVersion::V1,
        vec![IgnoreSource::at_root("data/\n!data/keep.csv\n")],
    )
    .unwrap();
    assert!(!legacy.includes_file("data/keep.csv").unwrap());
}

#[test]
fn a_broad_reinclusion_explores_every_branch_it_might_reach() {
    let root = scratch("broad");
    write(&root, "build/a/b/c.keep", b"k");
    write(&root, "build/a/b/c.o", b"o");
    write(&root, "build/z.o", b"o");
    listed::take();
    let result = evaluate(&root, &policy("build/\n!*.keep\n"));
    assert_eq!(tracked(&result), strings(&["build/a/b/c.keep"]));
    assert_eq!(listed_dirs(), strings(&["", "build", "build/a", "build/a/b"]));
}

#[test]
fn the_last_matching_rule_over_the_path_and_its_ancestors_decides() {
    let star = policy("*\n!*.rs\n");
    assert!(decide(&star, "src/main.rs", PathKind::File).is_tracked(), "Git 과 달리 `*` 뒤의 `!*.rs` 가 이긴다");
    assert!(is_excluded(&star, "src/notes.txt"));
    let dir_only = policy("cache/\n");
    assert!(decide(&dir_only, "cache", PathKind::File).is_tracked(), "끝의 / 는 디렉터리만");
    assert!(is_excluded(&dir_only, "cache/x"));
    assert!(is_excluded(&dir_only, "a/cache/x"), "/ 없는 패턴은 어느 깊이든");
    let anchored = policy("/out/\n");
    assert!(!is_excluded(&anchored, "a/out/x"));
    assert!(is_excluded(&anchored, "out/x"));
    assert!(matches!(
        decide(&star, "a/.gil", PathKind::Directory),
        Decision::NestedGilBoundary { .. }
    ));
    assert!(matches!(
        decide(&star, "sub/.gilignore", PathKind::File),
        Decision::NestedPolicyFile { .. }
    ));
}

#[test]
fn skipping_a_directory_requires_that_no_later_reinclusion_can_reach_inside() {
    use crate::artifact::tracking_policy::compile_line;
    let components = |text: &str| compile_line(text, 1).unwrap().unwrap().components;
    let below = |text: &str, dir: &[&str]| may_match_below(&components(text), dir);
    assert!(below("**/x", &["a", "b"]));
    assert!(below("x", &["a"]), "/ 없는 패턴은 어느 깊이에서도 맞을 수 있다");
    assert!(below("a/b", &["a"]));
    assert!(!below("a/b", &["c"]));
    assert!(!below("/a", &["a"]), "그 디렉터리 자신에만 맞고 아래에는 맞지 않는다");
    assert!(below("a", &["a"]), "/ 없는 `a` 는 a/a 에도 맞는다");
    assert!(below("a/**", &["a"]));
    assert!(below("a/*/c", &["a", "b"]));
    assert!(!below("a/*/c", &["a", "b", "c"]), "정확히 c 에서 끝난다");
    assert!(below("a/[bc]/d", &["a", "c"]));
}

// ── 연결 파일 ──────────────────────────────────────────────────────────────

#[cfg(unix)]
mod links {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn an_excluded_link_is_skipped_without_following_its_target() {
        let outside = scratch("link-outside");
        write(&outside, "secret/inside.txt", b"outside the project");
        let root = scratch("link-excluded");
        write(&root, "src/main.rs", b"x");
        symlink(&outside, root.join(".venv")).unwrap();
        symlink(outside.join("secret"), root.join("cache-link")).unwrap();
        listed::take();
        let result = evaluate(&root, &policy(".venv/\ncache-link\n"));
        assert_eq!(tracked(&result), strings(&["src/main.rs"]));
        for path in [".venv", "cache-link"] {
            let link = entry(&result, path);
            assert_eq!(link.kind, EntryKind::Link);
            assert_eq!(link.interior, None);
        }
        assert_eq!(listed_dirs(), strings(&["", "src"]), "연결 대상을 열지 않았다");
    }

    #[test]
    fn a_tracked_or_reincluded_link_is_refused() {
        let root = scratch("link-tracked");
        write(&root, "src/main.rs", b"x");
        symlink("main.rs", root.join("src/alias.rs")).unwrap();
        let err = refused(&root, &policy(""));
        assert_eq!(err.code(), "tracked_link");
        assert!(matches!(err, EvaluationError::TrackedLink { by: None, .. }));

        let root = scratch("link-reincluded");
        write(&root, "links/real.txt", b"x");
        symlink("real.txt", root.join("links/l")).unwrap();
        let err = refused(&root, &policy("links/\n!links/l\n"));
        match err {
            EvaluationError::TrackedLink { path, by: Some(rule) } => {
                assert_eq!(path.as_str(), "links/l");
                assert_eq!(rule.line, 2);
            }
            other => panic!("재포함한 연결 파일을 거절하지 않았다: {other:?}"),
        }
        // 디렉터리 전용 포함 예외가 더 아래 줄이면 그쪽이 이긴다 — 따라가서 실현하지 않는다.
        let root = scratch("link-dir-reincluded");
        write(&root, "real/x", b"x");
        symlink("real", root.join("venv")).unwrap();
        assert_eq!(refused(&root, &policy("venv\n!venv/\n")).code(), "tracked_link");
    }

    #[test]
    fn a_link_on_a_reinclusion_path_is_refused_not_followed() {
        let outside = scratch("link-path-outside");
        write(&outside, "keep.csv", b"k");
        let root = scratch("link-path");
        write(&root, "data/drop.csv", b"d");
        symlink(&outside, root.join("data/link")).unwrap();
        listed::take();
        let err = refused(&root, &policy("data/\n!data/link/keep.csv\n"));
        match &err {
            EvaluationError::LinkOnReinclusionPath {
                path,
                excluded_by,
                because,
            } => {
                assert_eq!(path.as_str(), "data/link");
                assert_eq!(excluded_by.line, 1);
                assert_eq!(because.iter().map(|r| r.line).collect::<Vec<_>>(), vec![2]);
            }
            other => panic!("재포함 경로 위의 연결 파일을 따라갔거나 건너뛰었다: {other:?}"),
        }
        assert!(!listed_dirs().iter().any(|dir| dir.starts_with("data/link")));
    }

    #[test]
    fn a_root_internal_store_link_and_a_root_link_are_refused() {
        let outside = scratch("gil-link-target");
        let root = scratch("gil-link");
        symlink(&outside, root.join(".gil")).unwrap();
        assert_eq!(refused(&root, &policy("")).code(), "internal_store_link");

        let real = scratch("root-real");
        write(&real, "a.txt", b"x");
        let parent = scratch("root-link-parent");
        symlink(&real, parent.join("project")).unwrap();
        assert_eq!(refused(&parent.join("project"), &policy("")).code(), "root_unavailable");
        assert_eq!(refused(&parent.join("project/"), &policy("")).code(), "root_unavailable", "끝의 / 로 연결을 풀지 않는다");
    }

    #[test]
    fn a_linked_nested_policy_file_is_a_blocker_and_is_not_read() {
        let outside = scratch("policy-link-target");
        write(&outside, "rules", b"*\n");
        let root = scratch("policy-link");
        write(&root, "sub/a.txt", b"a");
        symlink(outside.join("rules"), root.join("sub/.gilignore")).unwrap();
        symlink(outside.join("rules"), root.join(".gilignore")).unwrap();
        let result = evaluate(&root, &policy(""));
        assert_eq!(
            result.blockers(),
            &[Blocker::NestedPolicyFile {
                path: EntryPath::parse("sub/.gilignore").unwrap(),
                kind: EntryKind::Link,
            }]
        );
        let root_file = entry(&result, ".gilignore");
        assert_eq!(root_file.kind, EntryKind::Link);
        assert_eq!(root_file.reason, ExclusionReason::System(Protection::PolicyFile));
        assert_eq!(tracked(&result), strings(&["sub/a.txt"]));
    }

    #[test]
    fn a_special_entry_in_the_tracked_area_is_refused() {
        let root = scratch("fifo");
        write(&root, "a.txt", b"x");
        let fifo = std::ffi::CString::new(root.join("pipe").as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert_eq!(refused(&root, &policy("")).code(), "special_entry");
        // 직접 만난 특수 항목은 제외 규칙에 맞아도 거절한다 — 합의는 특수 파일 거절의 완화가 아니다.
        assert_eq!(refused(&root, &policy("pipe\n")).code(), "special_entry");
    }
}

#[cfg(windows)]
mod links {
    use super::*;

    /// 권한 없이 만들 수 있는 junction. 경로는 환경 값으로만 건넨다.
    fn junction(link: &Path, target: &Path) {
        let shell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let made = std::process::Command::new(shell)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:GIL_TEST_LINK -Target $env:GIL_TEST_TARGET | Out-Null",
            ])
            .env("GIL_TEST_LINK", link)
            .env("GIL_TEST_TARGET", target)
            .output()
            .unwrap();
        assert!(made.status.success(), "junction fixture creation failed");
    }

    #[test]
    fn an_excluded_junction_is_skipped_without_following_its_target() {
        let outside = scratch("junction-outside");
        write(&outside, "inside.txt", b"outside the project");
        let root = scratch("junction-excluded");
        write(&root, "src/main.rs", b"x");
        junction(&root.join(".venv"), &outside);
        listed::take();
        let result = evaluate(&root, &policy(".venv/\n"));
        assert_eq!(tracked(&result), strings(&["src/main.rs"]));
        assert_eq!(entry(&result, ".venv").kind, EntryKind::Link);
        assert_eq!(listed_dirs(), strings(&["", "src"]), "junction 대상을 열지 않았다");
    }

    #[test]
    fn a_tracked_or_reincluded_junction_is_refused() {
        let outside = scratch("junction-tracked-target");
        write(&outside, "keep.csv", b"k");
        let root = scratch("junction-tracked");
        junction(&root.join("linked"), &outside);
        assert_eq!(refused(&root, &policy("")).code(), "tracked_link");

        let root = scratch("junction-path");
        write(&root, "data/drop.csv", b"d");
        junction(&root.join("data/link"), &outside);
        assert_eq!(
            refused(&root, &policy("data/\n!data/link/keep.csv\n")).code(),
            "link_on_reinclusion_path"
        );
    }

    #[test]
    fn a_junction_inside_a_skipped_folder_is_not_seen() {
        let outside = scratch("junction-pruned-target");
        let root = scratch("junction-pruned");
        write(&root, "node_modules/pkg/index.js", b"x");
        junction(&root.join("node_modules/.bin"), &outside);
        listed::take();
        let result = evaluate(&root, &policy("node_modules/\n"));
        assert!(tracked(&result).is_empty());
        assert_eq!(entry(&result, "node_modules").interior, Some(Interior::Skip));
        assert_eq!(listed_dirs(), strings(&[""]), "node_modules 를 열지 않았다");
    }
}

// ── 하위 정책 파일 ─────────────────────────────────────────────────────────

#[test]
fn a_nested_policy_file_found_during_needed_exploration_blocks_and_is_not_inherited() {
    let root = scratch("nested-policy");
    write(&root, "sub/.gilignore", b"*\n");
    write(&root, "sub/a.txt", b"a");
    write(&root, "data/.gilignore", b"*.csv\n");
    write(&root, "data/x.csv", b"x");
    write(&root, "data/y.txt", b"y");
    let result = evaluate(&root, &policy("data/\n!data/*.csv\n"));
    let blocked: Vec<_> = result.blockers().iter().map(|b| (b.code(), b.path().to_string())).collect();
    assert_eq!(
        blocked,
        vec![
            ("nested_policy_file", "data/.gilignore".to_string()),
            ("nested_policy_file", "sub/.gilignore".to_string()),
        ]
    );
    assert!(result.is_blocked());
    // 하위 규칙을 상속하지 않았다: sub/.gilignore 의 `*` 와 data/.gilignore 의 `*.csv` 는 효과가 없다.
    assert_eq!(tracked(&result), strings(&["data/x.csv", "sub/a.txt"]));
    assert!(!tracked(&result).iter().any(|path| path.ends_with(".gilignore")));
    let text = result.blockers()[1].to_string();
    assert!(text.contains("sub/.gilignore") && text.contains("루트"), "{text}");
}

#[test]
fn a_nested_gil_boundary_met_during_needed_exploration_is_refused() {
    let root = scratch("nested-gil");
    write(&root, "sub/.gil/state.yaml", b"format: 4\n");
    assert_eq!(refused(&root, &policy("")).code(), "nested_gil_boundary");
    // 제외됐지만 재포함 때문에 들어간 폴더에서 만나도 거절한다.
    assert_eq!(refused(&root, &policy("sub/\n!sub/*.txt\n")).code(), "nested_gil_boundary");
    // 완전히 제외된 폴더 안이면 보지 않는다 — 검사했다고 주장하지도 않는다.
    let result = evaluate(&root, &policy("sub/\n"));
    assert_eq!(entry(&result, "sub").interior, Some(Interior::Skip));
}

// ── 잘못된 규칙 ────────────────────────────────────────────────────────────

#[test]
fn invalid_rules_are_all_rejected_with_line_and_reason() {
    let text = "# 주석\n[abc\nok/\nsrc\\gen\n../x\n!.git/hooks\n\\#literal\n\\!bang\nfoo\\\n";
    let err = rejected(text.as_bytes());
    let got: Vec<(usize, &str)> = err.problems.iter().map(|p| (p.line, p.reason.code())).collect();
    assert_eq!(
        got,
        vec![
            (2, "grammar"),
            (4, "backslash_not_escape"),
            (5, "dot_component"),
            (6, "reincludes_protected_only"),
            (9, "grammar"),
        ]
    );
    assert_eq!(err.problems[1].text, "src\\gen");
    assert_eq!(err.identity, policy("").identity().clone().with_same_engine(text.as_bytes()));
    let shown = err.to_string();
    assert!(shown.contains("4행") && shown.contains("5개 줄"), "{shown}");

    let codes = |err: &RulesRejected| -> Vec<(usize, &'static str)> {
        err.problems.iter().map(|p| (p.line, p.reason.code())).collect()
    };
    assert_eq!(codes(&rejected(b"\xEF\xBB\xBFnode_modules/\n")), vec![(1, "byte_order_mark")]);
    assert_eq!(codes(&rejected(b"ok/\nbad\xFF/\nfine\n")), vec![(2, "not_utf8")]);
    // 인코딩 문제가 다른 줄의 문제를 가리지 않는다.
    assert_eq!(codes(&rejected(b"[abc\nbad\xFF\n")), vec![(1, "grammar"), (2, "not_utf8")]);
    assert_eq!(
        codes(&rejected(b"\xEF\xBB\xBF[abc\nok\n../x\n")),
        vec![(1, "byte_order_mark"), (1, "grammar"), (3, "dot_component")]
    );
    assert_eq!(codes(&rejected(b"node_modules/\rdist/\r")), vec![(1, "carriage_return")]);
    assert!(ApprovedPolicy::compile(EngineVersion::V1, b"a\r\nb\r\n").is_ok(), "CRLF 는 받아들인다");

    // 문법상 특별한 글자의 escape 는 받아들인다.
    let escaped = policy("\\#literal\n\\!bang\nspace\\ \n");
    assert!(is_excluded(&escaped, "#literal"));
    assert!(is_excluded(&escaped, "!bang"));
    assert!(is_excluded(&escaped, "space "));
}

impl PolicyIdentity {
    /// 시험 도구 — 같은 엔진으로 다른 바이트의 정체성.
    fn with_same_engine(&self, bytes: &[u8]) -> PolicyIdentity {
        PolicyIdentity {
            engine: self.engine,
            rules: crate::artifact::digest_of(bytes).unwrap(),
        }
    }
}

// ── 추천 ───────────────────────────────────────────────────────────────────

fn compiled(item: &Recommendation) -> ApprovedPolicy {
    policy(&item.rules.join("\n"))
}

#[test]
fn recommendation_defaults_reasons_and_impacts() {
    let items = catalog();
    let by_key = |key: &str| items.iter().find(|item| item.key == key).unwrap_or_else(|| panic!("{key}"));
    for key in ["gil:other-vcs", "gil:node-modules", "gil:python-venv", "gil:python-cache", "gil:os-metadata", "gil:secret-dotenv", "gil:secret-keys"] {
        assert!(by_key(key).preselected, "{key} 는 미리 선택된 제안이다");
    }
    for key in ["gil:build-dist", "gil:build-build", "gil:build-target", "gil:build-out", "gil:build-coverage"] {
        assert!(!by_key(key).preselected, "{key} 는 산출물일 수 있어 미선택이다");
        assert_eq!(by_key(key).category, RecommendationCategory::BuildOutput);
    }
    for item in &items {
        assert_eq!(item.preselected, item.category.preselected());
        assert_eq!(item.sources, vec![RecommendationSource::GilCatalog]);
        assert!(item.impacts.contains(&Impact::NotArchivedOrRestored), "{}", item.key);
        assert!(item.impacts.contains(&Impact::AccessUnchanged), "{}", item.key);
        assert!(!item.reason.to_string().is_empty());
        // 의존성 목록·고정 파일은 어떤 카탈로그 추천으로도 제외되지 않는다.
        let rules = compiled(item);
        for name in DEPENDENCY_MANIFESTS {
            for at in [name.to_string(), format!("app/{name}")] {
                assert!(!is_excluded(&rules, &at), "{} 가 {at} 을(를) 제외한다", item.key);
            }
        }
    }
    assert!(by_key("gil:node-modules").impacts.contains(&Impact::ManifestsStayTracked));
    // 카탈로그는 포함 예외를 쓰지 않는다 — `!` 줄은 제외 폴더의 탐색을 강제한다.
    assert!(items.iter().all(|item| item.rules.iter().all(|rule| !rule.starts_with('!'))));
    assert!(by_key("gil:secret-keys").impacts.contains(&Impact::NameBasedSecretGuess));
    for notice in system_protections() {
        assert!(!notice.selectable(), "{notice:?}");
    }
    assert_eq!(system_protections().len(), 3);
}

/// 미리 선택된 추천을 모두 채택해도 의존성·캐시 폴더는 그대로 생략된다 — 순서와 무관하게.
#[test]
fn adopting_every_preselected_recommendation_keeps_dependency_folders_skipped() {
    let chosen: Vec<String> = catalog()
        .into_iter()
        .filter(|item| item.preselected)
        .flat_map(|item| item.rules)
        .collect();
    let root = scratch("preselected");
    write(&root, "package.json", b"{}");
    write(&root, "node_modules/typescript/bin/tsc", b"x");
    write(&root, "node_modules/pkg/.env.example", b"x");
    write(&root, ".venv/lib/site.py", b"x");
    write(&root, ".env.example", b"KEY=");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("../typescript/bin/tsc", root.join("node_modules/typescript/tsc-link")).unwrap();
        fs::create_dir_all(root.join(".venv/bin")).unwrap();
        std::os::unix::fs::symlink("python3", root.join(".venv/bin/python")).unwrap();
    }
    for rules in [chosen.clone(), chosen.iter().rev().cloned().collect()] {
        listed::take();
        let result = evaluate(&root, &policy(&rules.join("\n")));
        assert_eq!(tracked(&result), strings(&[".env.example", "package.json"]));
        for dir in ["node_modules", ".venv"] {
            assert_eq!(entry(&result, dir).interior, Some(Interior::Skip), "{dir}");
        }
        assert_eq!(listed_dirs(), strings(&[""]));
    }
}

#[test]
fn dotenv_recommendation_keeps_example_files_tracked() {
    let dotenv = catalog().into_iter().find(|item| item.key == "gil:secret-dotenv").unwrap();
    let rules = compiled(&dotenv);
    for path in [".env", ".env.local", ".env.production", ".env.staging", "api/.env", "api/.env.development.local"] {
        assert!(is_excluded(&rules, path), "{path} 는 비밀값 후보다");
    }
    // 예시 파일은 어느 규칙에도 맞지 않는다. 열거하지 않은 이름(.env.prod)도 후보가 아니다 — 이름 기반의 한계.
    for path in [".env.example", ".env.sample", ".env.template", "api/.env.example", ".envrc", "env.example", "config.example.json", ".env.prod"] {
        assert!(!is_excluded(&rules, path), "{path} 는 추적을 유지한다");
    }
    assert!(dotenv.impacts.iter().any(|impact| matches!(
        impact,
        Impact::ExampleFilesStayTracked { names } if names == &[".env.example", ".env.sample", ".env.template"]
    )));
    assert!(dotenv.impacts.contains(&Impact::NameBasedSecretGuess));
    let keys = catalog().into_iter().find(|item| item.key == "gil:secret-keys").unwrap();
    let keys = compiled(&keys);
    assert!(is_excluded(&keys, "certs/server.pem"));
    assert!(is_excluded(&keys, "id_ed25519"));
    assert!(!is_excluded(&keys, "id_ed25519.pub"), "공개 키는 후보가 아니다");
    assert!(!is_excluded(&keys, "slides/talk.key"), "`*.key` 는 Keynote 문서와 겹쳐 넣지 않았다");
}

#[test]
fn gitignore_is_only_a_reference_for_unselected_recommendations() {
    let review = review_gitignore(
        "# deps\nnode_modules/\n.env\nresults/\n!results/summary.csv\n[bad\n.git\npackage-lock.json\n\n",
    );
    let item = |key: &str| review.recommendations.iter().find(|item| item.key == key).unwrap();
    assert!(item("gil:node-modules").sources.contains(&RecommendationSource::Gitignore { line: 2 }));
    assert!(item("gil:node-modules").preselected, "카탈로그 기본값을 따른다");
    assert!(item("gil:secret-dotenv").sources.contains(&RecommendationSource::Gitignore { line: 3 }));
    let results = item("gitignore:4");
    assert_eq!(results.category, RecommendationCategory::FromGitignore);
    assert!(!results.preselected, "Git 에서 제외했다는 이유만으로 선택하지 않는다");
    assert_eq!(results.rules, strings(&["results/"]));
    assert_eq!(results.reason, Reason::ExcludedByGit);
    let lock = item("gitignore:8");
    assert!(lock.impacts.iter().any(|impact| matches!(
        impact,
        Impact::ExcludesDependencyManifests { names } if names == &["package-lock.json"]
    )));
    let skipped: Vec<usize> = review.skipped.iter().map(|line| line.line).collect();
    assert_eq!(skipped, vec![5, 6, 7]);

    // 문법과 같은 뒤 공백 처리, 머리의 BOM, 대표 위치의 의존성 목록 경고.
    let review = review_gitignore("\u{feff}node_modules/\nfoo\\ \nfrontend/\n");
    let item = |key: &str| review.recommendations.iter().find(|item| item.key == key).unwrap();
    assert!(item("gil:node-modules").sources.contains(&RecommendationSource::Gitignore { line: 1 }));
    assert_eq!(item("gitignore:2").rules, strings(&["foo\\ "]));
    assert!(item("gitignore:3").impacts.iter().any(|impact| matches!(
        impact,
        Impact::ExcludesDependencyManifests { names } if names.contains(&"package.json")
    )));
    assert!(review.skipped.is_empty(), "{:?}", review.skipped);

    // 추천은 적용 정책이 아니다 — 선택된 추천도 승인 원문에 들어오기 전에는 효과가 없다.
    let root = scratch("recommend-separate");
    write(&root, ".gitignore", b"node_modules/\n.env\n");
    write(&root, "node_modules/pkg/index.js", b"x");
    write(&root, ".env", b"TOKEN=synthetic-value-not-a-real-secret");
    let result = evaluate(&root, &policy(""));
    assert_eq!(tracked(&result), strings(&[".env", ".gitignore", "node_modules/pkg/index.js"]));
    assert_eq!(result.policy(), policy("").identity());
}

#[test]
fn project_specific_recommendations_start_unselected_and_are_checked() {
    let item = project_specific(
        "baseline",
        strings(&["data/baseline/", "!data/baseline/README.md"]),
        "실험 되돌리기와 독립적으로 쓰는 기준 자료".to_string(),
    )
    .unwrap();
    assert_eq!(item.category, RecommendationCategory::ProjectSpecific);
    assert!(!item.preselected);
    assert_eq!(item.sources, vec![RecommendationSource::Agent]);
    assert_eq!(item.key, "agent:baseline");
    let err = project_specific("bad", strings(&["[oops"]), String::new()).unwrap_err();
    assert_eq!(err.problems[0].reason.code(), "grammar");
    let manifest = project_specific("lock", strings(&["*.lock"]), String::new()).unwrap();
    assert!(manifest.impacts.iter().any(|i| i.code() == "excludes_dependency_manifests"));
}

#[test]
fn evaluation_results_never_contain_file_contents() {
    let root = scratch("no-contents");
    let marker = "SYNTHETIC-MARKER-7f3a";
    write(&root, ".env", format!("TOKEN={marker}").as_bytes());
    write(&root, "keys/id_rsa", marker.as_bytes());
    write(&root, "src/a.txt", marker.as_bytes());
    let result = evaluate(&root, &policy(".env\nkeys/\n"));
    assert!(!format!("{result:?}").contains(marker));
    assert!(!format!("{:?}", catalog()).contains(marker));
}

// ── 안정 관측과 기존 경로 ──────────────────────────────────────────────────

#[test]
fn a_change_between_the_two_scans_is_refused() {
    let root = scratch("changed");
    write(&root, "a.txt", b"x");
    let approved = policy("");
    let err = super::walk::evaluate_twice(&root, &approved, || write(&root, "b.txt", b"y"))
        .expect_err("두 훑기 사이의 변화를 받아들였다");
    assert_eq!(err.code(), "changed_during_evaluation");
}

#[test]
fn the_product_observer_and_its_whole_tree_semantics_are_untouched() {
    let root = scratch("product-observe");
    write(&root, ".gilignore", b"node_modules/\n.env\n");
    write(&root, ".env", b"x");
    write(&root, "node_modules/pkg/index.js", b"x");
    write(&root, "src/main.rs", b"x");
    let manifest = crate::artifact::observe(&root).expect("기본 관측");
    let paths: Vec<String> = manifest.paths().map(|p| p.to_string()).collect();
    // 기본 관측은 여전히 루트 .gil/ 만 뺀다 — .gilignore 를 읽지도 적용하지도 않는다.
    assert_eq!(paths, strings(&[".env", ".gilignore", "node_modules/pkg/index.js", "src/main.rs"]));
}
