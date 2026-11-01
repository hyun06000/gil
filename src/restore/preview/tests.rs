//! 미리보기 시험 — **아무것도 바꾸지 않는가, 현재에만 있는 것을 지키는가, 거절해야 할 것을 거절하는가.**

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::artifact::ManifestEntry;

static TICKET: AtomicU64 = AtomicU64::new(0);

fn digest(seed: u8) -> ContentDigest {
    crate::artifact::digest_from_raw([seed; 32])
}

fn path(text: &str) -> EntryPath {
    EntryPath::parse(text).unwrap_or_else(|| panic!("{text:?} 는 정규화된 경로여야 한다"))
}

fn manifest(entries: &[(&str, u8)]) -> Manifest {
    Manifest::new(
        entries
            .iter()
            .map(|(text, seed)| ManifestEntry {
                path: path(text),
                content: digest(*seed),
            })
            .collect(),
    )
}

/// 모두 추적하는 합성 정책.
fn include_all(_: &str) -> Result<bool, String> {
    Ok(true)
}

fn request<'a>(before: &'a Manifest, target: &'a Manifest, os: TargetOs) -> Request<'a> {
    Request {
        before,
        target,
        before_policy: Some("policy:test-1"),
        target_policy: Some("policy:test-1"),
        os,
    }
}

fn entry<'a>(preview: &'a Preview, text: &str) -> &'a Entry {
    preview
        .entries()
        .iter()
        .find(|entry| entry.path.as_str() == text)
        .unwrap_or_else(|| panic!("{text:?} 가 미리보기에 없다"))
}

fn blocks(preview: &Preview) -> Vec<Block> {
    match preview.readiness() {
        Readiness::Candidate => Vec::new(),
        Readiness::Refused(blocks) => blocks,
    }
}

// ── 역할 ───────────────────────────────────────────────────────────────────

#[test]
fn a_current_only_file_is_preserved_never_deleted() {
    let before = manifest(&[("keep.txt", 1), ("edit.txt", 2), ("mine-only.txt", 3)]);
    let target = manifest(&[("keep.txt", 1), ("edit.txt", 9), ("new.txt", 4)]);
    let preview = preview(&request(&before, &target, TargetOs::Unix), include_all, &NoProbe);

    assert_eq!(entry(&preview, "keep.txt").disposition, Disposition::Unchanged);
    assert_eq!(entry(&preview, "edit.txt").disposition, Disposition::WouldReplace);
    assert_eq!(entry(&preview, "new.txt").disposition, Disposition::WouldCreate);
    assert_eq!(
        entry(&preview, "mine-only.txt").disposition,
        Disposition::CurrentOnly {
            proposal: Preservation::QuarantineProposed,
        }
    );
    let tally = preview.tally();
    assert_eq!((tally.unchanged, tally.would_create, tally.would_replace, tally.current_only), (1, 1, 1, 1));

    // 사람이 읽는 꼴에도 삭제라는 말이 없고, 복원 명령도 없다.
    let text = preview.to_string();
    for forbidden in ["삭제", "Delete", "backup", "gil restore", "gil revisit"] {
        assert!(!text.contains(forbidden), "{forbidden:?} 가 미리보기 출력에 있다:\n{text}");
    }
    assert!(text.contains("보존 필요"));
}

#[test]
fn git_is_always_excluded_without_asking_the_policy() {
    use std::cell::RefCell;
    let asked: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let before = manifest(&[(".git/HEAD", 1), (".git/objects/ab", 2), ("src/a.rs", 3), (".gitignore", 4)]);
    let target = manifest(&[(".git/HEAD", 7), ("src/a.rs", 3), (".gitignore", 4)]);
    let preview = preview(
        &request(&before, &target, TargetOs::Unix),
        |text| {
            asked.borrow_mut().push(text.to_string());
            Ok(true)
        },
        &NoProbe,
    );

    for git in [".git/HEAD", ".git/objects/ab"] {
        assert!(
            matches!(
                entry(&preview, git).disposition,
                Disposition::Excluded {
                    by: Exclusion::GitAlways,
                    ..
                }
            ),
            "{git} 가 제외되지 않았다"
        );
        assert!(entry(&preview, git).risks.is_empty());
    }
    // `.gitignore` 는 `.git/` 이 아니다 — 정책에 묻는다.
    assert_eq!(entry(&preview, ".gitignore").disposition, Disposition::Unchanged);
    let asked = asked.into_inner();
    assert!(!asked.iter().any(|p| p.starts_with(".git/")), "정책에 .git 을 물었다: {asked:?}");
    assert!(asked.contains(&".gitignore".to_string()));
    assert!(asked.contains(&"src/a.rs".to_string()));
}

#[test]
fn a_policy_excluded_current_file_is_excluded_not_preserved_and_not_deleted() {
    let before = manifest(&[("node_modules/x.js", 1), ("src/a.rs", 2)]);
    let target = manifest(&[("src/a.rs", 2)]);
    let preview = preview(
        &request(&before, &target, TargetOs::Unix),
        |text| Ok(!text.starts_with("node_modules/")),
        &NoProbe,
    );
    assert_eq!(
        entry(&preview, "node_modules/x.js").disposition,
        Disposition::Excluded {
            by: Exclusion::Policy,
            side: Side::BeforeOnly,
        }
    );
    assert!(entry(&preview, "node_modules/x.js").risks.is_empty());
}

#[test]
fn a_matching_policy_whose_target_contains_an_excluded_path_is_inconsistent() {
    // 같은 정책으로 관측했다면 목표 세계에 제외 경로가 있을 수 없다.
    let before = manifest(&[("src/a.rs", 2)]);
    let target = manifest(&[("src/a.rs", 2), ("node_modules/x.js", 1)]);
    let preview = preview(
        &request(&before, &target, TargetOs::Unix),
        |text| Ok(!text.starts_with("node_modules/")),
        &NoProbe,
    );
    let entry = entry(&preview, "node_modules/x.js");
    assert!(matches!(entry.disposition, Disposition::Excluded { by: Exclusion::Policy, side: Side::TargetOnly }));
    assert_eq!(entry.risks, vec![Risk::PolicyInconsistent]);
    assert!(blocks(&preview).iter().any(|b| matches!(b, Block::Path { risk: Risk::PolicyInconsistent, .. })));
}

#[test]
fn a_policy_error_refuses_that_path_and_the_whole_candidate() {
    let before = manifest(&[("ok.txt", 1), ("weird.txt", 2)]);
    let target = manifest(&[("ok.txt", 1), ("weird.txt", 3)]);
    let preview = preview(
        &request(&before, &target, TargetOs::Unix),
        |text| {
            if text == "weird.txt" {
                Err("규칙 7 을 해석할 수 없다".to_string())
            } else {
                Ok(true)
            }
        },
        &NoProbe,
    );
    let weird = entry(&preview, "weird.txt");
    // 역할은 그대로 보이되(정보성 diff), 위험이 붙는다.
    assert_eq!(weird.disposition, Disposition::WouldReplace);
    assert_eq!(weird.risks, vec![Risk::PolicyError("규칙 7 을 해석할 수 없다".to_string())]);
    assert!(weird.blocked(TargetOs::Unix));
    assert!(entry(&preview, "ok.txt").risks.is_empty());
    assert!(blocks(&preview).iter().any(|b| matches!(
        b,
        Block::Path { path, risk: Risk::PolicyError(_) } if path == "weird.txt"
    )));
}

#[test]
fn the_internal_store_is_never_a_restore_target() {
    let before = manifest(&[(".gil/state.yaml", 1)]);
    let target = manifest(&[(".gil/state.yaml", 2)]);
    let preview = preview(&request(&before, &target, TargetOs::Unix), include_all, &NoProbe);
    assert!(entry(&preview, ".gil/state.yaml").risks.contains(&Risk::InternalStore));
}

// ── 정책 식별자 ────────────────────────────────────────────────────────────

#[test]
fn different_or_unknown_policy_ids_still_give_a_diff_but_never_a_candidate() {
    let before = manifest(&[("a.txt", 1), ("gone.txt", 2)]);
    let target = manifest(&[("a.txt", 1), ("b.txt", 3)]);

    let mismatch = Request {
        before: &before,
        target: &target,
        before_policy: Some("policy:now"),
        target_policy: Some("policy:then"),
        os: TargetOs::Unix,
    };
    let preview_a = preview(&mismatch, include_all, &NoProbe);
    assert_eq!(
        preview_a.policy(),
        &PolicyCheck::Mismatch {
            before: "policy:now".to_string(),
            target: "policy:then".to_string(),
        }
    );
    // 정보성 diff 는 그대로 있다.
    assert_eq!(entry(&preview_a, "b.txt").disposition, Disposition::WouldCreate);
    assert!(matches!(entry(&preview_a, "gone.txt").disposition, Disposition::CurrentOnly { .. }));
    assert!(blocks(&preview_a).iter().any(|b| matches!(b, Block::PolicyMismatch { .. })));

    for (before_policy, target_policy) in [(None, Some("policy:then")), (Some("policy:now"), None), (None, None)] {
        let unknown = Request {
            before: &before,
            target: &target,
            before_policy,
            target_policy,
            os: TargetOs::Unix,
        };
        let preview_b = preview(&unknown, include_all, &NoProbe);
        assert_eq!(
            preview_b.policy(),
            &PolicyCheck::Unknown {
                before_known: before_policy.is_some(),
                target_known: target_policy.is_some(),
            }
        );
        assert!(
            blocks(&preview_b).iter().any(|b| matches!(b, Block::PolicyUnknown { .. })),
            "{before_policy:?}/{target_policy:?} 가 legacy 로 표시되지 않았다"
        );
        assert!(preview_b.to_string().contains("legacy"));
    }
}

#[test]
fn without_a_file_system_probe_the_preview_is_explicitly_not_a_candidate() {
    let same = manifest(&[("a.txt", 1)]);
    let preview = preview(&request(&same, &same, TargetOs::Unix), include_all, &NoProbe);
    assert!(matches!(preview.probe(), ProbeStatus::Unsupported { adapter: "none", .. }));
    assert_eq!(
        blocks(&preview),
        vec![Block::FileSystemNotProbed {
            why: "파일 시스템 조회를 요청하지 않았다".to_string(),
        }]
    );
}

// ── 경로 글자 ──────────────────────────────────────────────────────────────

#[test]
fn unsafe_text_is_caught_even_where_entry_path_already_refuses_it() {
    let cases: [(&str, Risk); 6] = [
        ("../out.txt", Risk::DotComponent),
        ("a/./b", Risk::DotComponent),
        ("/abs.txt", Risk::EmptyComponent),
        ("a//b", Risk::EmptyComponent),
        ("//?/C:/x", Risk::EmptyComponent),
        ("a\\b", Risk::Backslash),
    ];
    for (text, risk) in cases {
        let risks = inspect_text(text);
        assert!(risks.contains(&risk), "{text:?} → {risks:?} 에 {risk:?} 가 없다");
        assert_eq!(risk.severity(TargetOs::Unix), Severity::Blocks);
        assert_eq!(risk.severity(TargetOs::Windows), Severity::Blocks);
    }
    assert!(inspect_text("a\u{0}b").contains(&Risk::Nul));
    assert!(inspect_text("//?/C:/x").contains(&Risk::WindowsDrive) == false, "`//?/` 뒤의 드라이브는 첫 조각이 아니다");
    let long = "x".repeat(256);
    assert!(inspect_text(&long).contains(&Risk::ComponentTooLong { bytes: 256 }));
}

#[test]
fn a_path_that_was_fine_on_macos_but_impossible_on_windows_is_judged_separately() {
    let cases: [(&str, Risk); 8] = [
        ("C:/x.txt", Risk::WindowsDrive),
        ("notes:draft.txt", Risk::WindowsAlternateStream),
        ("what?.txt", Risk::WindowsForbiddenChar('?')),
        ("a<b", Risk::WindowsForbiddenChar('<')),
        ("ctrl\u{1}.txt", Risk::WindowsControlChar),
        ("trailing. ", Risk::WindowsTrailingDotOrSpace),
        ("docs/CON.txt", Risk::WindowsReservedName("CON".to_string())),
        ("com¹", Risk::WindowsReservedName("COM¹".to_string())),
    ];
    for (text, risk) in cases {
        let before = manifest(&[]);
        let target = manifest(&[(text, 1)]);
        let on_mac = preview(&request(&before, &target, TargetOs::MacOs), include_all, &NoProbe);
        let on_windows = preview(&request(&before, &target, TargetOs::Windows), include_all, &NoProbe);
        assert!(entry(&on_mac, text).risks.contains(&risk), "{text:?} → {:?}", entry(&on_mac, text).risks);
        // macOS 에서는 정보성 표식, Windows 에서는 막힘.
        assert_eq!(risk.severity(TargetOs::MacOs), Severity::ElsewhereOnly(TargetOs::Windows));
        assert_eq!(risk.severity(TargetOs::Windows), Severity::Blocks);
        assert!(!entry(&on_mac, text).blocked(TargetOs::MacOs), "{text:?} 가 macOS 에서 막혔다");
        assert!(entry(&on_windows, text).blocked(TargetOs::Windows));
        assert!(on_mac.to_string().contains("Windows 에서만 막힘"));
    }
    // `nul` 은 대소문자 없이, 확장자가 붙어도 예약이다. `nullable.rs` 는 아니다.
    assert!(windows_reserved("nul").is_some());
    assert!(windows_reserved("Nul.txt").is_some());
    assert!(windows_reserved("LPT9.log").is_some());
    assert!(windows_reserved("nullable.rs").is_none());
    assert!(windows_reserved("COM0").is_none());
    assert!(windows_reserved("COM10").is_none());
}

#[test]
fn ordinary_korean_and_ascii_paths_carry_no_risk() {
    let before = manifest(&[("문서/보고서.md", 1), ("src/lib.rs", 2)]);
    let target = manifest(&[("문서/보고서.md", 3), ("src/lib.rs", 2), ("데이터/표.csv", 4)]);
    for os in [TargetOs::Unix, TargetOs::MacOs, TargetOs::Windows] {
        let preview = preview(&request(&before, &target, os), include_all, &NoProbe);
        for entry in preview.entries() {
            assert!(entry.risks.is_empty(), "{os}: {:?} → {:?}", entry.path, entry.risks);
        }
    }
}

// ── 겹침 ───────────────────────────────────────────────────────────────────

#[test]
fn case_collisions_block_only_where_the_file_system_folds_case() {
    // 목표가 만들 `Readme.md` 와 현재에만 있어 보존되는 `readme.md` 가 같은 자리를 다툰다.
    let before = manifest(&[("readme.md", 1)]);
    let target = manifest(&[("Readme.md", 2)]);
    for (os, blocks_here) in [(TargetOs::Unix, false), (TargetOs::MacOs, true), (TargetOs::Windows, true)] {
        let preview = preview(&request(&before, &target, os), include_all, &NoProbe);
        let created = entry(&preview, "Readme.md");
        let preserved = entry(&preview, "readme.md");
        assert_eq!(created.risks, vec![Risk::CaseCollision { with: vec!["readme.md".to_string()] }]);
        assert_eq!(preserved.risks, vec![Risk::CaseCollision { with: vec!["Readme.md".to_string()] }]);
        assert_eq!(created.blocked(os), blocks_here, "{os}");
        assert!(matches!(preserved.disposition, Disposition::CurrentOnly { .. }));
    }
}

#[test]
fn hangul_syllables_and_jamo_are_the_same_name_on_macos() {
    // 「한글.txt」 — 음절형과 조합형 자모. APFS/HFS+ 는 같은 이름으로 본다.
    let composed = "한글.txt";
    let decomposed = "\u{1112}\u{1161}\u{11AB}\u{1100}\u{1173}\u{11AF}.txt";
    assert_ne!(composed, decomposed);
    assert_eq!(hangul_decomposed(composed), decomposed);

    let before = manifest(&[(decomposed, 1)]);
    let target = manifest(&[(composed, 2)]);
    let on_mac = preview(&request(&before, &target, TargetOs::MacOs), include_all, &NoProbe);
    assert_eq!(
        entry(&on_mac, composed).risks,
        vec![Risk::NormalizationCollision { with: vec![decomposed.to_string()] }]
    );
    assert!(entry(&on_mac, composed).blocked(TargetOs::MacOs));

    let on_unix = preview(&request(&before, &target, TargetOs::Unix), include_all, &NoProbe);
    assert_eq!(
        entry(&on_unix, composed).risks[0].severity(TargetOs::Unix),
        Severity::ElsewhereOnly(TargetOs::MacOs)
    );
    assert!(!entry(&on_unix, composed).blocked(TargetOs::Unix));
}

#[test]
fn latin_combining_marks_are_unverified_not_silently_passed() {
    // `é` 를 e + 결합 acute 로 적은 이름 — 정규화 표 없이 `é` 와의 동등성을 판단하지 못한다.
    let decomposed = "cafe\u{301}.txt";
    let before = manifest(&[]);
    let target = manifest(&[(decomposed, 1)]);
    let on_mac = preview(&request(&before, &target, TargetOs::MacOs), include_all, &NoProbe);
    assert_eq!(entry(&on_mac, decomposed).risks, vec![Risk::UnverifiedNormalization]);
    assert_eq!(Risk::UnverifiedNormalization.severity(TargetOs::MacOs), Severity::Unverified);
    assert!(entry(&on_mac, decomposed).blocked(TargetOs::MacOs));
    assert!(on_mac.to_string().contains("미확인"));
    // 정규화에 민감한 파일 시스템에서는 다른 이름일 뿐이다.
    assert_eq!(Risk::UnverifiedNormalization.severity(TargetOs::Unix), Severity::ElsewhereOnly(TargetOs::MacOs));
}

#[test]
fn a_file_and_a_directory_cannot_share_a_name() {
    // 현재에만 있는 파일 `a` 는 보존되는데 목표는 `a/b.txt` 를 만들려 한다 — 모든 OS 에서 막힌다.
    let before = manifest(&[("a", 1)]);
    let target = manifest(&[("a/b.txt", 2)]);
    for os in [TargetOs::Unix, TargetOs::MacOs, TargetOs::Windows] {
        let preview = preview(&request(&before, &target, os), include_all, &NoProbe);
        assert_eq!(
            entry(&preview, "a").risks,
            vec![Risk::TopologyConflict {
                with: "a/b.txt".to_string(),
                kind: Topology::FileWhereDirectoryNeeded,
            }]
        );
        assert_eq!(
            entry(&preview, "a/b.txt").risks,
            vec![Risk::TopologyConflict {
                with: "a".to_string(),
                kind: Topology::DirectoryWhereFileExists,
            }]
        );
        assert!(entry(&preview, "a").blocked(os));
    }

    // 대소문자만 다른 디렉터리/파일 겹침은 접는 OS 에서만.
    let before = manifest(&[("Docs", 1)]);
    let target = manifest(&[("docs/x.md", 2)]);
    let on_unix = preview(&request(&before, &target, TargetOs::Unix), include_all, &NoProbe);
    assert!(entry(&on_unix, "Docs").risks.is_empty(), "{:?}", entry(&on_unix, "Docs").risks);
    let on_mac = preview(&request(&before, &target, TargetOs::MacOs), include_all, &NoProbe);
    assert!(entry(&on_mac, "Docs").risks.iter().any(|r| matches!(r, Risk::TopologyConflict { .. })));
}

// ── 결정성 ─────────────────────────────────────────────────────────────────

#[test]
fn the_same_input_gives_the_same_preview_in_ascending_path_order() {
    let before = manifest(&[("z.txt", 1), ("가.txt", 2), ("A/b.txt", 3), ("a.txt", 4), ("Readme", 5)]);
    let target = manifest(&[("readme", 6), ("a.txt", 4), ("가.txt", 7), ("b/c.txt", 8)]);
    let first = preview(&request(&before, &target, TargetOs::MacOs), include_all, &NoProbe);
    let second = preview(&request(&before, &target, TargetOs::MacOs), include_all, &NoProbe);
    assert_eq!(first, second);
    assert_eq!(first.fingerprint(), second.fingerprint());

    let names: Vec<&str> = first.entries().iter().map(|e| e.path.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    assert_eq!(names, sorted, "경로 UTF-8 바이트 오름차순이 아니다");
    assert_eq!(names, vec!["A/b.txt", "Readme", "a.txt", "b/c.txt", "readme", "z.txt", "가.txt"]);

    // 입력의 쪽만 바꿔도 지문이 다르다 — 정책 식별자 하나만 달라져도.
    let other_policy = Request {
        before: &before,
        target: &target,
        before_policy: Some("policy:test-1"),
        target_policy: Some("policy:test-2"),
        os: TargetOs::MacOs,
    };
    assert_ne!(preview(&other_policy, include_all, &NoProbe).fingerprint(), first.fingerprint());
    // callback 의 답이 달라져도.
    let third = preview(&request(&before, &target, TargetOs::MacOs), |t| Ok(t != "z.txt"), &NoProbe);
    assert_ne!(third.fingerprint(), first.fingerprint());
}

#[test]
fn a_preview_knows_when_the_world_it_saw_has_moved_on() {
    let before = manifest(&[("a.txt", 1)]);
    let target = manifest(&[("a.txt", 2)]);
    let preview = preview(&request(&before, &target, TargetOs::Unix), include_all, &NoProbe);
    assert!(preview.still_matches(&before));
    assert!(!preview.still_matches(&manifest(&[("a.txt", 1), ("later.txt", 3)])));
    assert!(!preview.still_matches(&manifest(&[("a.txt", 5)])));
}

// ── 디스크 조회 (읽기 전용) ─────────────────────────────────────────────────

/// 임시 프로젝트 — 큐가 준 TMPDIR 아래에 선다.
fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "gil-preview-{label}-{}-{}",
        std::process::id(),
        TICKET.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let at = root.join(relative);
    fs::create_dir_all(at.parent().unwrap()).unwrap();
    fs::write(at, bytes).unwrap();
}

/// 폴더 전체의 (경로 → 바이트 또는 링크 대상·종류). `.gil` 도 포함해 **전부** 잰다.
fn bytes_of(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for item in fs::read_dir(at).unwrap() {
            let item = item.unwrap();
            let path = item.path();
            let kind = fs::symlink_metadata(&path).unwrap().file_type();
            let relative = path.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            if kind.is_symlink() {
                let to = fs::read_link(&path).unwrap();
                out.insert(format!("{relative} -> "), to.to_string_lossy().into_owned().into_bytes());
            } else if kind.is_dir() {
                out.insert(format!("{relative}/"), Vec::new());
                walk(root, &path, out);
            } else {
                out.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

#[cfg(unix)]
fn observed(root: &Path) -> Manifest {
    crate::artifact::observe(root).expect("합성 프로젝트는 관측된다")
}

#[cfg(unix)]
#[test]
fn a_probed_preview_changes_nothing_on_disk_byte_for_byte() {
    let root = scratch("byte-identical");
    write(&root, "src/main.rs", b"fn main() {}\n");
    write(&root, "notes/todo.md", "- 하나\n".as_bytes());
    write(&root, "only-here.bin", &[0u8, 1, 2, 3]);
    write(&root, ".gitignore", b"target/\n");
    write(&root, ".git/HEAD", b"ref: refs/heads/main\n");
    write(&root, ".gil/state.yaml", b"format: 7\n");
    write(&root, ".gil/objects/ab/cdef", b"blob bytes");
    let frozen = bytes_of(&root);

    let before = observed(&root);
    // 목표: main.rs 가 다르고, todo.md 는 없고, 새 파일 하나가 더 있고, only-here.bin 은 없다.
    let mut target_entries: Vec<ManifestEntry> = before
        .entries()
        .iter()
        .filter(|e| e.path.as_str() != "notes/todo.md" && e.path.as_str() != "only-here.bin")
        .cloned()
        .collect();
    for entry in target_entries.iter_mut() {
        if entry.path.as_str() == "src/main.rs" {
            entry.content = digest(9);
        }
    }
    target_entries.push(ManifestEntry {
        path: path("docs/new.md"),
        content: digest(8),
    });
    let target = Manifest::new(target_entries);

    let probe = LstatProbe::at(&root);
    let result = preview(
        &request(&before, &target, TargetOs::host()),
        |t| Ok(!t.starts_with("target/")),
        &probe,
    );

    assert_eq!(bytes_of(&root), frozen, "미리보기가 디스크를 바꿨다");

    assert_eq!(result.probe(), &ProbeStatus::Checked { adapter: "unix-lstat" });
    assert_eq!(entry(&result, "src/main.rs").disposition, Disposition::WouldReplace);
    assert_eq!(entry(&result, "docs/new.md").disposition, Disposition::WouldCreate);
    assert!(matches!(entry(&result, "notes/todo.md").disposition, Disposition::CurrentOnly { .. }));
    assert!(matches!(entry(&result, "only-here.bin").disposition, Disposition::CurrentOnly { .. }));
    assert!(matches!(entry(&result, ".git/HEAD").disposition, Disposition::Excluded { by: Exclusion::GitAlways, .. }));
    // 깨끗한 합성 폴더에 같은 정책 — 막는 것이 없다. 그래도 「복원 허용」이 아니다.
    assert_eq!(result.readiness(), Readiness::Candidate, "{result}");
    assert!(result.to_string().contains("복원 허용이 아니다"));
    assert!(result.still_matches(&observed(&root)));

    // 미리보기 뒤 다시 관측해도 같다.
    assert_eq!(bytes_of(&root), frozen);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn a_file_that_appeared_after_observation_makes_the_preview_stale() {
    let root = scratch("stale");
    write(&root, "a.txt", b"a");
    let before = observed(&root);
    let target = manifest(&[("a.txt", 1), ("b.txt", 2)]);

    // 관측 뒤에 누군가 b.txt 를 만들었다 — Manifest 는 「없다」고 아는데 디스크에 있다.
    write(&root, "b.txt", b"surprise");
    let frozen = bytes_of(&root);
    let result = preview(&request(&before, &target, TargetOs::host()), include_all, &LstatProbe::at(&root));
    assert_eq!(bytes_of(&root), frozen);
    assert_eq!(
        entry(&result, "b.txt").risks,
        vec![Risk::Probe(ProbeFinding::Stale {
            expected: EntryKind::Missing,
            found: EntryKind::File,
        })]
    );
    assert!(!result.still_matches(&observed(&root)));
    assert!(blocks(&result).iter().any(|b| matches!(b, Block::Path { risk: Risk::Probe(ProbeFinding::Stale { .. }), .. })));
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn symlinks_and_case_variants_on_disk_are_seen_not_followed() {
    use std::os::unix::fs::symlink;
    let root = scratch("probe-findings");
    write(&root, "real/x.txt", b"x");
    write(&root, "Dup.txt", b"d");
    let outside = scratch("probe-outside");
    write(&outside, "secret", b"s");
    symlink(&outside, root.join("linked")).unwrap();
    symlink("real/x.txt", root.join("alias.txt")).unwrap();
    let frozen = bytes_of(&root);
    let frozen_outside = bytes_of(&outside);

    // 관측은 링크를 거절하므로 before 를 손으로 든다 — 과거 Manifest 가 그런 경로를 담았을 수 있다.
    let before = manifest(&[("real/x.txt", 1)]);
    let target = manifest(&[("real/x.txt", 1), ("linked/new.txt", 2), ("alias.txt", 3), ("dup.txt", 4)]);
    let result = preview(&request(&before, &target, TargetOs::MacOs), include_all, &LstatProbe::at(&root));

    assert_eq!(bytes_of(&root), frozen);
    assert_eq!(bytes_of(&outside), frozen_outside);
    assert_eq!(
        entry(&result, "linked/new.txt").risks,
        vec![Risk::Probe(ProbeFinding::SymlinkAncestor { at: "linked".to_string() })]
    );
    assert_eq!(entry(&result, "alias.txt").risks, vec![Risk::Probe(ProbeFinding::SymlinkLeaf)]);
    assert_eq!(
        entry(&result, "dup.txt").risks,
        vec![Risk::Probe(ProbeFinding::CaseVariantOnDisk { name: "Dup.txt".to_string() })]
    );
    assert!(entry(&result, "dup.txt").blocked(TargetOs::MacOs));
    assert_eq!(Risk::Probe(ProbeFinding::CaseVariantOnDisk { name: String::new() }).basis(), Basis::FileSystem);
    assert!(result.to_string().contains("디스크 조회"));
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&outside);
}

#[cfg(not(unix))]
#[test]
fn on_this_os_the_lstat_probe_refuses_instead_of_pretending() {
    let root = scratch("unsupported");
    let same = manifest(&[("a.txt", 1)]);
    let result = preview(&request(&same, &same, TargetOs::host()), include_all, &LstatProbe::at(&root));
    assert!(matches!(result.probe(), ProbeStatus::Unsupported { adapter: "unsupported-on-this-os", .. }));
    assert!(blocks(&result).iter().any(|b| matches!(b, Block::FileSystemNotProbed { .. })));
    let _ = fs::remove_dir_all(&root);
}

// ── Plan 과 섞이지 않는다 ──────────────────────────────────────────────────

#[test]
fn a_preview_carries_no_backup_ordinal_no_delete_and_no_world_number() {
    // 기존 Plan 은 Delete 와 보관 서수를 들고, 세계 번호를 담는다. 미리보기는 그 어느 것도 없다 —
    // 같은 두 Manifest 를 넣어도 결과 타입이 다르고, 서로 변환하는 문이 없다.
    let before = manifest(&[("gone.txt", 1), ("edit.txt", 2)]);
    let target = manifest(&[("edit.txt", 3)]);
    let root = Path::new("/이런/자리는/없다");
    let plan_steps = super::super::plan::steps(root, &before, &target);
    assert!(plan_steps
        .iter()
        .any(|s| matches!(s.operation, super::super::Operation::Delete { .. })), "기존 Plan 은 Delete 를 계획한다");

    let preview = preview(&request(&before, &target, TargetOs::Unix), include_all, &NoProbe);
    assert!(matches!(entry(&preview, "gone.txt").disposition, Disposition::CurrentOnly { .. }));
    let debug = format!("{preview:?}");
    for word in ["Delete", "backup", "world", "SnapshotRef"] {
        assert!(!debug.contains(word), "미리보기 안에 {word:?} 가 있다");
    }
}
