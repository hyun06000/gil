//! 추천 — **제안이지 정책이 아니다.**
//!
//! 여기서 나오는 것은 승인 화면(4·5번 작업)이 보여 줄 후보다. 미리 선택됐다는 사실도 승인이나
//! 적용이 아니다. 이 모듈은 [`ApprovedPolicy`](super::ApprovedPolicy) 를 만들지도 바꾸지도 않으며,
//! 파일을 읽거나 쓰지 않는다. `.gitignore` 원문은 호출자가 **추천 참고 자료로** 건넬 때만 본다.
//!
//! | 분류 | 기본 선택 | 근거 (Artifact Model §3.6, 2026-11-19 확정) |
//! |---|---|---|
//! | 시스템 보호 (`.git`, 루트 `.gil`, 루트 `.gilignore`) | 선택 대상이 아님 | 사용자 선택으로 해제할 수 없다 |
//! | 다른 도구의 내부 기록, 의존성 설치 폴더, 캐시, OS 메타데이터 | 선택됨 | 일반 추천은 미리 선택된 제안 |
//! | 비밀값이 들어갈 수 있는 파일 | 선택됨 | 비밀값 파일 추천도 미리 선택된 제안 |
//! | 빌드 출력 후보 (`dist/` 등) | 미선택 | 산출물일 수 있다 — 프로젝트별 자료·산출물은 미선택 |
//! | `.gitignore` 에서 온 후보 | 미선택 | Git 에서 제외했다는 이유만으로 선택·적용하지 않는다 |
//! | 에이전트의 프로젝트별 추천 | 미선택 | 에이전트가 추천했다는 사실만으로 넣지 않는다 |
//!
//! # 비밀값 후보와 예시 파일 — 이 엔진의 세부 패턴 (2026-11-20)
//!
//! - dotenv: `.env`, `.env.local`, `.env.*.local`, `.env.development`, `.env.production`, `.env.staging`,
//!   `.env.test` 를 **열거한다.** `.env.example`·`.env.sample`·`.env.template` 같은 예시 파일은 어느 규칙에도
//!   맞지 않아 추적을 유지한다. 예시라는 이름만으로 내용이 안전하다고 단정하지 않는다 — 예시 파일에 실제
//!   값이 있으면 사용자가 따로 제외해야 한다.
//! - **카탈로그는 포함 예외(`!`)를 쓰지 않는다.** `/` 없는 `!` 줄은 어느 깊이에도 맞을 수 있어, 미리 선택된
//!   추천을 함께 채택하면 `node_modules/` 같은 제외 폴더를 모두 탐색하게 만든다(그 안의 연결 파일 때문에
//!   평가가 거절될 수도 있다). 그래서 `.env.*` + `!.env.example` 대신 열거를 택했다. `.env.prod` 처럼
//!   열거하지 않은 이름은 후보가 아니다.
//! - 개인 키 이름: `*.pem`, `*.p12`, `*.pfx`, `id_rsa`, `id_dsa`, `id_ecdsa`, `id_ed25519`.
//!   `*.key` 는 넣지 않았다 — macOS Keynote 문서 등 프로젝트 자료와 겹친다.
//! - **이름 기반 추천이다.** 모든 비밀값을 찾지 못하고(`.envrc`, 설정 파일 안의 토큰 등), 과거
//!   보관본의 내용을 지우지 않는다. 파일 내용을 읽지 않으므로 추천 설명에 비밀값이 들어갈 수 없다.
//!
//! 의존성 폴더 추천은 디렉터리 패턴만 쓴다. [`DEPENDENCY_MANIFESTS`] 의 목록·고정 파일은 어떤
//! 카탈로그 규칙에도 맞지 않으며, 다른 출처의 후보가 그 파일을 제외하면 영향에 경고를 붙인다.

use std::fmt;

use super::{ApprovedPolicy, Decision, EngineVersion, PathKind, Protection, RulesRejected};
use crate::artifact::EntryPath;

/// 의존성 목록·버전 고정 파일 — 의존성 폴더를 추천해도 **함께 제외하지 않는다.**
pub(crate) const DEPENDENCY_MANIFESTS: &[&str] = &[
    "package.json",
    "package-lock.json",
    "npm-shrinkwrap.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lockb",
    "Cargo.toml",
    "Cargo.lock",
    "requirements.txt",
    "pyproject.toml",
    "poetry.lock",
    "Pipfile",
    "Pipfile.lock",
    "uv.lock",
    "go.mod",
    "go.sum",
    "Gemfile",
    "Gemfile.lock",
    "composer.json",
    "composer.lock",
];

/// dotenv 추천의 규칙에 맞지 않아 추적을 유지하는 예시 파일 이름.
const DOTENV_EXAMPLES: &[&str] = &[".env.example", ".env.sample", ".env.template"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecommendationCategory {
    ToolInternal,
    Dependencies,
    Cache,
    OsMetadata,
    Secrets,
    BuildOutput,
    FromGitignore,
    ProjectSpecific,
}

impl RecommendationCategory {
    pub(crate) fn code(self) -> &'static str {
        match self {
            RecommendationCategory::ToolInternal => "tool_internal",
            RecommendationCategory::Dependencies => "dependencies",
            RecommendationCategory::Cache => "cache",
            RecommendationCategory::OsMetadata => "os_metadata",
            RecommendationCategory::Secrets => "secrets",
            RecommendationCategory::BuildOutput => "build_output",
            RecommendationCategory::FromGitignore => "from_gitignore",
            RecommendationCategory::ProjectSpecific => "project_specific",
        }
    }

    /// 새 추천의 **초기 표시**. 기존 승인 정책이나 사용자 선택을 바꾸는 값이 아니다.
    pub(crate) fn preselected(self) -> bool {
        match self {
            RecommendationCategory::ToolInternal
            | RecommendationCategory::Dependencies
            | RecommendationCategory::Cache
            | RecommendationCategory::OsMetadata
            | RecommendationCategory::Secrets => true,
            RecommendationCategory::BuildOutput
            | RecommendationCategory::FromGitignore
            | RecommendationCategory::ProjectSpecific => false,
        }
    }
}

/// 추천이 어디서 왔는가 — 적용 규칙과 따로 남는다(계약 §5.1: 출처는 제안 원장에, 정책에는 없다).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecommendationSource {
    GilCatalog,
    /// 호출자가 건넨 루트 `.gitignore` 의 몇째 줄.
    Gitignore { line: usize },
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reason {
    OtherToolInternal,
    ReinstallableDependencies,
    RegenerableCache,
    OsMetadata,
    MayHoldSecrets,
    MayBeBuildOutput,
    ExcludedByGit,
    /// 에이전트가 적은 작업상 이유. 비밀값을 넣지 않는 것은 호출자의 책임이다.
    Project(String),
}

impl Reason {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Reason::OtherToolInternal => "other_tool_internal",
            Reason::ReinstallableDependencies => "reinstallable_dependencies",
            Reason::RegenerableCache => "regenerable_cache",
            Reason::OsMetadata => "os_metadata",
            Reason::MayHoldSecrets => "may_hold_secrets",
            Reason::MayBeBuildOutput => "may_be_build_output",
            Reason::ExcludedByGit => "excluded_by_git",
            Reason::Project(_) => "project",
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reason::OtherToolInternal => f.write_str("다른 버전 관리 도구의 내부 기록이다"),
            Reason::ReinstallableDependencies => {
                f.write_str("설치된 의존성은 목록·버전 고정 파일에서 다시 설치할 수 있다")
            }
            Reason::RegenerableCache => f.write_str("도구가 다시 만드는 캐시다"),
            Reason::OsMetadata => f.write_str("운영체제가 만드는 폴더 표시 정보다"),
            Reason::MayHoldSecrets => {
                f.write_str("비밀값이 들어갈 수 있는 이름이다. 보관본에 비밀값을 복제하지 않으려는 제안이다")
            }
            Reason::MayBeBuildOutput => f.write_str(
                "빌드 출력에 흔한 이름이지만 이 프로젝트의 산출물일 수도 있어 미리 선택하지 않았다",
            ),
            Reason::ExcludedByGit => f.write_str(
                "Git 이 무시하는 규칙이다. GIL 의 보관 범위는 따로 정하며, Git 에서 뺀 자료가 GIL 에는 필요할 수 있다",
            ),
            Reason::Project(why) => f.write_str(why),
        }
    }
}

/// 제외했을 때 보관·복원에 생기는 일.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Impact {
    NotArchivedOrRestored,
    AccessUnchanged,
    ManifestsStayTracked,
    NameBasedSecretGuess,
    ExampleFilesStayTracked { names: Vec<&'static str> },
    /// 이 후보의 규칙이 의존성 목록·고정 파일을 제외한다 — 막지는 않고 알린다.
    ExcludesDependencyManifests { names: Vec<&'static str> },
}

impl Impact {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Impact::NotArchivedOrRestored => "not_archived_or_restored",
            Impact::AccessUnchanged => "access_unchanged",
            Impact::ManifestsStayTracked => "manifests_stay_tracked",
            Impact::NameBasedSecretGuess => "name_based_secret_guess",
            Impact::ExampleFilesStayTracked { .. } => "example_files_stay_tracked",
            Impact::ExcludesDependencyManifests { .. } => "excludes_dependency_manifests",
        }
    }
}

impl fmt::Display for Impact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Impact::NotArchivedOrRestored => f.write_str(
                "제외하면 이후 새 보관본에 이 경로의 내용이 들어가지 않고, GIL 복원이 이 경로를 만들거나 \
                 바꾸거나 지우지 않는다. 그 기간의 내용은 GIL 로 되돌릴 수 없다",
            ),
            Impact::AccessUnchanged => f.write_str(
                "제외는 읽기·수정·삭제 금지나 파일 불변성을 뜻하지 않는다",
            ),
            Impact::ManifestsStayTracked => {
                f.write_str("의존성 목록·버전 고정 파일은 계속 추적한다")
            }
            Impact::NameBasedSecretGuess => f.write_str(
                "파일 이름으로 고른 추천이라 모든 비밀값을 찾지 못하며, 이미 만든 보관본의 내용을 지우지 않는다",
            ),
            Impact::ExampleFilesStayTracked { names } => write!(
                f,
                "예시 파일({})은 추적을 유지한다. 예시라는 이름만으로 내용이 안전하다고 보지 않으므로 실제 \
                 값이 있으면 따로 제외한다",
                names.join(", ")
            ),
            Impact::ExcludesDependencyManifests { names } => write!(
                f,
                "의존성 목록·버전 고정 파일({})도 제외된다",
                names.join(", ")
            ),
        }
    }
}

/// 하나의 추천 — 대상 규칙·출처·이유·기본 선택·영향.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Recommendation {
    /// 화면과 제안 원장이 선택 상태를 묶는 안정된 키.
    pub(crate) key: String,
    pub(crate) category: RecommendationCategory,
    /// 채택하면 `.gilignore` 에 들어갈 줄들. 채택은 승인 절차의 일이다.
    pub(crate) rules: Vec<String>,
    pub(crate) sources: Vec<RecommendationSource>,
    pub(crate) preselected: bool,
    pub(crate) reason: Reason,
    pub(crate) impacts: Vec<Impact>,
}

/// 선택 대상이 아닌 시스템 보호의 안내.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SystemProtectionNotice {
    pub(crate) protection: Protection,
    pub(crate) shown: &'static str,
}

impl SystemProtectionNotice {
    /// 언제나 거짓 — 사용자 선택으로 해제할 수 없다.
    pub(crate) fn selectable(&self) -> bool {
        false
    }
}

pub(crate) fn system_protections() -> Vec<SystemProtectionNotice> {
    vec![
        SystemProtectionNotice {
            protection: Protection::GitMetadata,
            shown: ".git (어느 깊이든)",
        },
        SystemProtectionNotice {
            protection: Protection::InternalStore,
            shown: ".gil (프로젝트 루트)",
        },
        SystemProtectionNotice {
            protection: Protection::PolicyFile,
            shown: ".gilignore (프로젝트 루트)",
        },
    ]
}

fn item(
    key: &str,
    category: RecommendationCategory,
    rules: &[&str],
    reason: Reason,
    extra: Vec<Impact>,
) -> Recommendation {
    let mut impacts = vec![Impact::NotArchivedOrRestored, Impact::AccessUnchanged];
    impacts.extend(extra);
    Recommendation {
        key: key.to_string(),
        category,
        rules: rules.iter().map(|rule| rule.to_string()).collect(),
        sources: vec![RecommendationSource::GilCatalog],
        preselected: category.preselected(),
        reason,
        impacts,
    }
}

/// GIL 이 내장한 일반 추천 목록. 프로젝트 폴더를 보지 않는 고정 목록이다.
pub(crate) fn catalog() -> Vec<Recommendation> {
    use RecommendationCategory as C;
    let mut items = vec![
        item("gil:other-vcs", C::ToolInternal, &[".svn/", ".hg/"], Reason::OtherToolInternal, vec![]),
        item(
            "gil:node-modules",
            C::Dependencies,
            &["node_modules/"],
            Reason::ReinstallableDependencies,
            vec![Impact::ManifestsStayTracked],
        ),
        item(
            "gil:python-venv",
            C::Dependencies,
            &[".venv/"],
            Reason::ReinstallableDependencies,
            vec![Impact::ManifestsStayTracked],
        ),
        item(
            "gil:python-cache",
            C::Cache,
            &["__pycache__/", ".pytest_cache/", ".mypy_cache/"],
            Reason::RegenerableCache,
            vec![],
        ),
        item("gil:os-metadata", C::OsMetadata, &[".DS_Store", "Thumbs.db"], Reason::OsMetadata, vec![]),
        item(
            "gil:secret-dotenv",
            C::Secrets,
            &[".env", ".env.local", ".env.*.local", ".env.development", ".env.production", ".env.staging", ".env.test"],
            Reason::MayHoldSecrets,
            vec![
                Impact::NameBasedSecretGuess,
                Impact::ExampleFilesStayTracked {
                    names: DOTENV_EXAMPLES.to_vec(),
                },
            ],
        ),
        item(
            "gil:secret-keys",
            C::Secrets,
            &["*.pem", "*.p12", "*.pfx", "id_rsa", "id_dsa", "id_ecdsa", "id_ed25519"],
            Reason::MayHoldSecrets,
            vec![Impact::NameBasedSecretGuess],
        ),
    ];
    for dir in ["dist", "build", "target", "out", "coverage"] {
        items.push(item(
            &format!("gil:build-{dir}"),
            C::BuildOutput,
            &[&format!("{dir}/")],
            Reason::MayBeBuildOutput,
            vec![],
        ));
    }
    items
}

/// 에이전트의 프로젝트별 추천 — **언제나 미선택이다.** 규칙은 엔진 문법으로 검사한다.
pub(crate) fn project_specific(
    key: &str,
    rules: Vec<String>,
    why: String,
) -> Result<Recommendation, RulesRejected> {
    ApprovedPolicy::compile(EngineVersion::V1, rules.join("\n").as_bytes())?;
    let mut impacts = vec![Impact::NotArchivedOrRestored, Impact::AccessUnchanged];
    impacts.extend(manifest_warning(&rules));
    Ok(Recommendation {
        key: format!("agent:{key}"),
        category: RecommendationCategory::ProjectSpecific,
        rules,
        sources: vec![RecommendationSource::Agent],
        preselected: RecommendationCategory::ProjectSpecific.preselected(),
        reason: Reason::Project(why),
        impacts,
    })
}

/// 의존성 목록·고정 파일을 대표 위치에서 확인한다 — 모든 위치를 증명하는 검사가 아니다.
const MANIFEST_PROBES: &[&str] = &["", "app/", "frontend/", "backend/", "packages/pkg/", "crates/pkg/"];

/// 규칙들이 대표 위치([`MANIFEST_PROBES`])의 의존성 목록·고정 파일을 제외하는가.
fn manifest_warning(rules: &[String]) -> Option<Impact> {
    let policy = ApprovedPolicy::compile(EngineVersion::V1, rules.join("\n").as_bytes()).ok()?;
    let names: Vec<&'static str> = DEPENDENCY_MANIFESTS
        .iter()
        .copied()
        .filter(|name| {
            MANIFEST_PROBES.iter().any(|dir| {
                let path = EntryPath::parse(&format!("{dir}{name}")).expect("고정 이름은 경로 계약을 지킨다");
                matches!(policy.decide(&path, PathKind::File), Decision::Excluded { .. })
            })
        })
        .collect();
    (!names.is_empty()).then_some(Impact::ExcludesDependencyManifests { names })
}

/// `.gitignore` 를 추천 참고 자료로 읽은 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitignoreReview {
    /// 카탈로그(같은 규칙이면 출처가 더해진다) + `.gitignore` 에서 온 미선택 후보.
    pub(crate) recommendations: Vec<Recommendation>,
    pub(crate) skipped: Vec<SkippedGitignoreLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkippedGitignoreLine {
    pub(crate) line: usize,
    pub(crate) text: String,
    pub(crate) why: &'static str,
}

/// 호출자가 건넨 **루트** `.gitignore` 원문을 추천으로 옮긴다. 적용하지 않는다.
///
/// - 같은 글자의 카탈로그 규칙이 있으면 그 추천에 출처만 더한다(기본 선택은 카탈로그를 따른다).
/// - 그 밖의 줄은 줄마다 미선택 후보가 된다.
/// - 포함 예외(`!`), 이 엔진 문법으로 해석할 수 없는 줄, 시스템 보호를 가리키는 줄은 옮기지 않고
///   `skipped` 에 이유와 함께 남긴다. 하위 폴더의 `.gitignore` 는 위치 의미가 달라 받지 않는다.
pub(crate) fn review_gitignore(text: &str) -> GitignoreReview {
    let mut recommendations = catalog();
    let mut skipped = Vec::new();
    // Git 은 머리의 UTF-8 BOM 을 받아들인다 — 참고 자료에서는 벗기고 읽는다.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for (index, raw) in text.split('\n').enumerate() {
        let line = index + 1;
        let shown = raw.strip_suffix('\r').unwrap_or(raw);
        // 문법과 같은 방식으로 뒤 공백을 지운다 — `\ ` 로 escape 한 공백은 남는다.
        let rule = crate::artifact::tracking_policy::trim_trailing_spaces(shown);
        if rule.is_empty() || rule.starts_with('#') {
            continue;
        }
        let skip = |why| SkippedGitignoreLine {
            line,
            text: shown.to_string(),
            why,
        };
        if rule.starts_with('!') {
            skipped.push(skip("포함 예외는 추천으로 옮기지 않는다"));
            continue;
        }
        if ApprovedPolicy::compile(EngineVersion::V1, rule.as_bytes()).is_err() {
            skipped.push(skip("GIL 정책 문법으로 해석할 수 없다"));
            continue;
        }
        let bare = rule.trim_start_matches('/').trim_end_matches('/');
        if [".git", ".gil", ".gilignore"].iter().any(|name| super::same_system_name(bare, name)) {
            skipped.push(skip("시스템 보호 대상이라 선택할 필요가 없다"));
            continue;
        }
        if let Some(known) = recommendations
            .iter_mut()
            .find(|item| item.sources.contains(&RecommendationSource::GilCatalog) && item.rules.iter().any(|r| r == rule))
        {
            known.sources.push(RecommendationSource::Gitignore { line });
            continue;
        }
        let rules = vec![rule.to_string()];
        let mut impacts = vec![Impact::NotArchivedOrRestored, Impact::AccessUnchanged];
        impacts.extend(manifest_warning(&rules));
        recommendations.push(Recommendation {
            key: format!("gitignore:{line}"),
            category: RecommendationCategory::FromGitignore,
            rules,
            sources: vec![RecommendationSource::Gitignore { line }],
            preselected: RecommendationCategory::FromGitignore.preselected(),
            reason: Reason::ExcludedByGit,
            impacts,
        });
    }
    GitignoreReview {
        recommendations,
        skipped,
    }
}
