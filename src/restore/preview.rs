//! 복원 **미리보기** — 파일을 하나도 바꾸지 않고 「되돌리면 무엇이 일어나는가」를 판정한다.
//!
//! ```text
//! before Manifest ─┐
//! target Manifest ─┼─→ preview() ─→ Preview   (경로마다 역할 + 위험 근거, 전체 준비 판정)
//! 정책 ID 둘      ─┤
//! 포함 여부 callback┘       아무것도 쓰지 않는다. 작업 폴더·state.yaml·객체 저장소 전부 그대로.
//! ```
//!
//! # 이것은 [`Plan`](super::Plan) 이 아니다
//!
//! 기존 `restore/plan.rs::steps` 는 **target 에 없는 현재 파일을 Delete 로 계획한다.** 이 모듈은
//! 그 함수를 부르지도, 그 타입으로 바뀌지도 않는다. 같은 두 Manifest 를 받아도 결과의 뜻이 다르다.
//!
//! ```text
//! Plan::Step  Replace / Delete / Create   실행하면 파일이 바뀐다 — 보관 서수가 붙는다
//! Preview     유지 / 생성 예정 / 변경 예정 / 현재에만 있음(보존) / 정책 제외 / 거절
//!             실행하는 문이 없다 — 보관 서수도, 복원 명령도 만들지 않는다
//! ```
//!
//! **현재에만 있는 파일은 삭제 대상이 아니다**(Artifact Model §3.6a 3항). 미리보기는 그 자리를
//! 「보존 필요 — 격리 제안」으로만 적는다. 격리 *제안*과 실제 파일 이동은 다른 일이고, 이동은
//! 여기 없다.
//!
//! # 정책은 밖에서 들어온다
//!
//! 추적 정책의 타입을 import 하지 않는다(Wave 1 INTERFACES). `Fn(&str) -> Result<bool, String>`
//! 하나와 before/target 의 정책 식별자 둘을 받는다. 식별자가 없거나 다르면 **정보성 diff 는
//! 내지만 apply 후보로는 판정하지 않는다** — 과거 Snapshot 에 지금 정책을 소급해 「같은 세계」라고
//! 말하는 것이 바로 숨은 삭제이기 때문이다.
//!
//! # 근거의 종류를 섞지 않는다
//!
//! ```text
//! Basis::Manifest    두 Manifest 의 글자만 보고 안 것 — 어느 기계에서 돌려도 같다
//! Basis::FileSystem  읽기 전용 adapter 가 지금 디스크를 들여다보고 안 것 — 관측 시점이 붙는다
//! ```
//!
//! 글자 검사는 `CON`·`a:b`·대소문자 겹침 같은 **표현의 위험**을 안다. 그것으로 Windows handle
//! 안전성이나 TOCTOU 를 보장한다고 말하지 않는다 — 그 둘은 적용 순간에 handle 기준으로 다시
//! 재야 하는 것이고, 이 모듈은 적용하지 않는다. 디스크 조회가 없는 환경이면 **조용히 통과시키지
//! 않고 「조회하지 않았다」로 거절한다.**
//!
//! 이 모듈이 완성되어도 제품 `gil restore` / `gil revisit` 의 `product-safety` 차단은 그대로다.
//! 안전 restore 전체는 저장 provenance·apply 연결·양 OS 실기까지 닫혀야 끝난다.

// 아직 crate 안의 아무도 이 미리보기를 부르지 않는다 — 통합 단계가 Session/명령에 연결한다.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::artifact::{ContentDigest, EntryPath, Manifest};

#[cfg(test)]
mod tests;

// ── 입력 ───────────────────────────────────────────────────────────────────

/// 미리보기가 겨냥하는 운영체제 — **경로 규칙이 여기서 갈린다.**
///
/// 같은 Manifest 라도 macOS 에서 유효했던 경로가 Windows 에서는 불가능할 수 있다. 어느 OS 의
/// 규칙으로 재는지 입력으로 받고, 그 OS 에서 막히는 것과 다른 OS 에서만 막히는 것을 따로 적는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TargetOs {
    /// 대소문자·정규화에 민감한 파일 시스템(ext4 등).
    Unix,
    /// 기본 APFS/HFS+ — 대소문자 무시, Unicode 정규화 무시.
    MacOs,
    /// NTFS — 대소문자 무시, 예약 이름·금지 문자·ADS 문법.
    Windows,
}

impl TargetOs {
    /// 지금 이 바이너리가 도는 OS.
    pub(crate) fn host() -> TargetOs {
        if cfg!(windows) {
            TargetOs::Windows
        } else if cfg!(target_os = "macos") {
            TargetOs::MacOs
        } else {
            TargetOs::Unix
        }
    }

    fn folds_case(self) -> bool {
        matches!(self, TargetOs::MacOs | TargetOs::Windows)
    }

    fn folds_normalization(self) -> bool {
        matches!(self, TargetOs::MacOs)
    }

    fn as_str(self) -> &'static str {
        match self {
            TargetOs::Unix => "Unix",
            TargetOs::MacOs => "macOS",
            TargetOs::Windows => "Windows",
        }
    }
}

impl fmt::Display for TargetOs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 미리보기에 넣는 것 전부.
///
/// `before` 는 지금 관측한 세계, `target` 은 돌아가려는 세계다. 정책 식별자는 **각 Manifest 가
/// 어느 정책으로 관측됐는가**이며, 없으면 `None` 으로 둔다 — 과거 Manifest 에 지금 정책의 이름을
/// 임의로 붙이지 않는다.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Request<'a> {
    pub(crate) before: &'a Manifest,
    pub(crate) target: &'a Manifest,
    pub(crate) before_policy: Option<&'a str>,
    pub(crate) target_policy: Option<&'a str>,
    pub(crate) os: TargetOs,
}

// ── 읽기 전용 파일 시스템 조회 ─────────────────────────────────────────────

/// 디스크의 한 자리가 지금 무엇인가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryKind {
    Missing,
    File,
    Directory,
    Symlink,
    Other,
}

/// 조회가 답하지 못한 이유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeError {
    /// 이 환경에서는 읽기 전용 조회를 제공하지 않는다 — **거절 사유다.** 통과가 아니다.
    Unsupported(String),
    /// 조회 자체가 실패했다.
    Io(String),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProbeError::Unsupported(why) => write!(f, "조회 미지원 — {why}"),
            ProbeError::Io(why) => write!(f, "조회 실패 — {why}"),
        }
    }
}

/// **읽기만 하는** 파일 시스템 adapter.
///
/// 미리보기는 이 문으로만 디스크를 본다. 만들고·지우고·옮기는 메서드는 없다 — 없어야 미리보기가
/// 「아무것도 바꾸지 않는다」를 타입으로 말할 수 있다.
///
/// 조회로 아는 것은 **조회한 순간**의 사실이다. 적용 순간에 같다고 보장하지 않는다.
pub(crate) trait Probe {
    /// 사람이 읽을 adapter 이름 — 결과에 「무엇으로 봤는가」로 남는다.
    fn describe(&self) -> &'static str;
    /// 프로젝트 상대 경로(`/` 구분) 한 자리의 종류. 뿌리는 `""` 다.
    fn kind(&self, relative: &str) -> Result<EntryKind, ProbeError>;
    /// 디렉터리 안의 이름들 — 대소문자 변형이 디스크에 있는지 볼 때 쓴다. 뿌리는 `""`.
    fn names(&self, relative_dir: &str) -> Result<Vec<String>, ProbeError>;
}

/// 조회를 하지 않는다 — 결과는 **명시적으로** 「파일 시스템을 보지 않았다」가 된다.
pub(crate) struct NoProbe;

impl Probe for NoProbe {
    fn describe(&self) -> &'static str {
        "none"
    }

    fn kind(&self, _relative: &str) -> Result<EntryKind, ProbeError> {
        Err(ProbeError::Unsupported(
            "파일 시스템 조회를 요청하지 않았다".to_string(),
        ))
    }

    fn names(&self, _relative_dir: &str) -> Result<Vec<String>, ProbeError> {
        Err(ProbeError::Unsupported(
            "파일 시스템 조회를 요청하지 않았다".to_string(),
        ))
    }
}

/// Unix 의 `lstat`/`readdir` 로만 보는 adapter.
///
/// 심볼릭 링크를 따라가지 않는다(`symlink_metadata`). 그래도 **경로 이름으로** 보는 것이라
/// 조회 뒤에 누가 바꾸면 모른다 — 그 한계는 결과에 적힌다.
#[cfg(unix)]
pub(crate) struct LstatProbe {
    root: std::path::PathBuf,
}

#[cfg(unix)]
impl LstatProbe {
    pub(crate) fn at(root: &std::path::Path) -> LstatProbe {
        LstatProbe {
            root: root.to_path_buf(),
        }
    }

    fn join(&self, relative: &str) -> std::path::PathBuf {
        if relative.is_empty() {
            self.root.clone()
        } else {
            self.root.join(relative)
        }
    }
}

#[cfg(unix)]
impl Probe for LstatProbe {
    fn describe(&self) -> &'static str {
        "unix-lstat"
    }

    fn kind(&self, relative: &str) -> Result<EntryKind, ProbeError> {
        match std::fs::symlink_metadata(self.join(relative)) {
            Ok(data) => {
                let kind = data.file_type();
                Ok(if kind.is_symlink() {
                    EntryKind::Symlink
                } else if kind.is_dir() {
                    EntryKind::Directory
                } else if kind.is_file() {
                    EntryKind::File
                } else {
                    EntryKind::Other
                })
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(EntryKind::Missing),
            Err(source) => Err(ProbeError::Io(source.to_string())),
        }
    }

    fn names(&self, relative_dir: &str) -> Result<Vec<String>, ProbeError> {
        let listing = match std::fs::read_dir(self.join(relative_dir)) {
            Ok(listing) => listing,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(ProbeError::Io(source.to_string())),
        };
        let mut names = Vec::new();
        for item in listing {
            let item = item.map_err(|source| ProbeError::Io(source.to_string()))?;
            match item.file_name().to_str() {
                Some(name) => names.push(name.to_string()),
                None => {
                    return Err(ProbeError::Io(format!(
                        "{relative_dir:?} 안에 UTF-8 로 적을 수 없는 이름이 있다"
                    )));
                }
            }
        }
        names.sort();
        Ok(names)
    }
}

/// Unix 가 아닌 곳의 기본 adapter — **handle 기반 조회가 서기 전까지 명시적으로 거절한다.**
///
/// `std::fs::symlink_metadata` 는 Windows 에서 reparse point 를 가르고 handle 을 고정하지
/// 못한다. 그것으로 본 것을 「확인했다」고 적으면 Windows 계약(§2–§4)을 글자로 흉내 내는 셈이다.
#[cfg(not(unix))]
pub(crate) struct LstatProbe;

#[cfg(not(unix))]
impl LstatProbe {
    pub(crate) fn at(_root: &std::path::Path) -> LstatProbe {
        LstatProbe
    }
}

#[cfg(not(unix))]
impl Probe for LstatProbe {
    fn describe(&self) -> &'static str {
        "unsupported-on-this-os"
    }

    fn kind(&self, _relative: &str) -> Result<EntryKind, ProbeError> {
        Err(ProbeError::Unsupported(
            "이 OS 의 handle 기반 읽기 전용 조회는 아직 없다 — 글자 검사만 했다".to_string(),
        ))
    }

    fn names(&self, _relative_dir: &str) -> Result<Vec<String>, ProbeError> {
        Err(ProbeError::Unsupported(
            "이 OS 의 handle 기반 읽기 전용 조회는 아직 없다 — 글자 검사만 했다".to_string(),
        ))
    }
}

// ── 결과 ───────────────────────────────────────────────────────────────────

/// 정책 식별자 둘을 견준 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PolicyCheck {
    /// 같은 정책으로 관측된 두 세계다.
    Matched { id: String },
    /// 한쪽 또는 양쪽에 정책 식별자가 없다 — **legacy Manifest.** 지금 정책을 소급하지 않는다.
    Unknown { before_known: bool, target_known: bool },
    /// 서로 다른 정책으로 관측됐다. 「같은 파일이 없다」가 삭제인지 제외인지 가를 수 없다.
    Mismatch { before: String, target: String },
}

/// 어느 세계에 있는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    BeforeOnly,
    TargetOnly,
    Both,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Side::BeforeOnly => "현재에만",
            Side::TargetOnly => "목표에만",
            Side::Both => "양쪽에",
        })
    }
}

/// 왜 복원이 건드리지 않는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Exclusion {
    /// `.git/` — 정책이 무엇이든 **언제나** 제외한다(§3.6a 1항). callback 을 묻지도 않는다.
    GitAlways,
    /// 주입된 정책이 제외했다.
    Policy,
}

/// 현재에만 있는 파일을 어떻게 다룰지에 대한 **제안.** 실행이 아니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Preservation {
    /// 복구 가능한 격리 자리로 옮기고 receipt 를 남기자는 제안. 옮기는 코드는 여기 없다.
    QuarantineProposed,
}

/// 한 경로가 복원에서 맡는 역할.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Disposition {
    /// 양쪽에 같은 내용 — 건드리지 않는다.
    Unchanged,
    /// 목표에만 있다 — 복원하면 만들어질 자리.
    WouldCreate,
    /// 양쪽에 있고 내용이 다르다 — 복원하면 바뀔 자리.
    WouldReplace,
    /// **현재에만 있다 — 보존 필요.** 삭제 대상으로 만들지 않는다.
    CurrentOnly { proposal: Preservation },
    /// 복원이 건드리지 않는 자리.
    Excluded { by: Exclusion, side: Side },
}

impl fmt::Display for Disposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Disposition::Unchanged => f.write_str("유지"),
            Disposition::WouldCreate => f.write_str("생성 예정"),
            Disposition::WouldReplace => f.write_str("변경 예정"),
            Disposition::CurrentOnly { .. } => f.write_str("현재에만 있음 — 보존 필요(격리 제안)"),
            Disposition::Excluded { by: Exclusion::GitAlways, side } => {
                write!(f, "제외 — .git 은 언제나 제외({side} 있음)")
            }
            Disposition::Excluded { by: Exclusion::Policy, side } => {
                write!(f, "제외 — 정책({side} 있음)")
            }
        }
    }
}

/// 두 자리가 어떻게 겹치는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Topology {
    /// 이 파일의 이름을 다른 경로가 디렉터리로 쓴다.
    FileWhereDirectoryNeeded,
    /// 이 경로의 조상 자리에 파일이 있다.
    DirectoryWhereFileExists,
}

/// 조회가 이 자리에서 본 것.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeFinding {
    /// 조상 가운데 심볼릭 링크가 있다 — 경로가 프로젝트 밖으로 향할 수 있다.
    SymlinkAncestor { at: String },
    /// 이 자리 자체가 심볼릭 링크다.
    SymlinkLeaf,
    /// 조상 자리에 디렉터리가 아닌 것이 있다.
    AncestorNotDirectory { at: String, kind: EntryKind },
    /// 이 자리에 일반 파일이 아닌 것이 있다.
    LeafNotFile { kind: EntryKind },
    /// `before` Manifest 가 본 것과 지금 디스크가 다르다 — **이 미리보기는 낡았다.**
    Stale { expected: EntryKind, found: EntryKind },
    /// 대소문자만 다른 이름이 같은 폴더에 있다.
    CaseVariantOnDisk { name: String },
    /// 조회가 답하지 못했다.
    Failed(ProbeError),
}

/// 한 경로에 붙는 위험 — **근거와 함께.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Risk {
    /// 정책 callback 이 오류를 돌려줬다.
    PolicyError(String),
    /// 정책이 같다는데 목표 세계가 제외 경로를 담았다 — 식별자가 거짓이거나 Manifest 가 섞였다.
    PolicyInconsistent,
    /// 루트 `.gil/` — 내부 저장소는 복원 대상이 아니다.
    InternalStore,
    /// NUL 바이트.
    Nul,
    /// `\` — 구분자 혼합. 공개 표현은 `/` 하나다.
    Backslash,
    /// 빈 조각 — 앞뒤 `/`, `//`, UNC/device 접두(`//?/`, `//./`).
    EmptyComponent,
    /// `.` 또는 `..` 조각.
    DotComponent,
    /// 조각 하나가 255 바이트를 넘는다 — 대부분의 파일 시스템 한계.
    ComponentTooLong { bytes: usize },
    /// `C:` 꼴 드라이브 상대 경로.
    WindowsDrive,
    /// 조각 안의 `:` — NTFS 대체 데이터 스트림 문법.
    WindowsAlternateStream,
    /// `< > " | ? *`.
    WindowsForbiddenChar(char),
    /// 0x01–0x1F 제어 문자.
    WindowsControlChar,
    /// 조각 끝의 `.` 또는 공백 — Windows 가 잘라 내 다른 이름이 된다.
    WindowsTrailingDotOrSpace,
    /// `CON`·`NUL`·`COM1`… 확장자가 붙어도 예약된다.
    WindowsReservedName(String),
    /// 조각이 UTF-16 255 단위를 넘는다.
    WindowsComponentTooLong { units: usize },
    /// 결합 문자가 있다 — 정규화 표 없이 NFC/NFD 동등성을 판단할 수 없다.
    UnverifiedNormalization,
    /// 대소문자만 다른 다른 경로가 결과 세계에 있다.
    CaseCollision { with: Vec<String> },
    /// Unicode 정규화만 다른 다른 경로가 결과 세계에 있다(한글 음절/자모는 알고리즘으로 판정).
    NormalizationCollision { with: Vec<String> },
    /// 파일/디렉터리 겹침.
    TopologyConflict { with: String, kind: Topology },
    /// 디스크 조회가 본 것.
    Probe(ProbeFinding),
}

/// 그 위험이 **이 OS 에서** 얼마나 무거운가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    /// 이 OS 에서 복원할 수 없다.
    Blocks,
    /// 판단할 근거가 모자란다 — 통과로 치지 않는다.
    Unverified,
    /// 이 OS 에서는 무해하고 다른 OS 에서만 막힌다 — legacy 호환 표식.
    ElsewhereOnly(TargetOs),
}

/// 그 위험을 무엇으로 알았는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Basis {
    /// Manifest 의 글자만으로.
    Manifest,
    /// 읽기 전용 디스크 조회로 — 조회 시점의 사실이다.
    FileSystem,
}

impl Risk {
    pub(crate) fn basis(&self) -> Basis {
        match self {
            Risk::Probe(_) => Basis::FileSystem,
            _ => Basis::Manifest,
        }
    }

    pub(crate) fn severity(&self, os: TargetOs) -> Severity {
        let windows_only = |os: TargetOs| {
            if os == TargetOs::Windows {
                Severity::Blocks
            } else {
                Severity::ElsewhereOnly(TargetOs::Windows)
            }
        };
        match self {
            Risk::PolicyError(_)
            | Risk::PolicyInconsistent
            | Risk::InternalStore
            | Risk::Nul
            | Risk::Backslash
            | Risk::EmptyComponent
            | Risk::DotComponent
            | Risk::ComponentTooLong { .. }
            | Risk::TopologyConflict { .. } => Severity::Blocks,
            Risk::WindowsDrive
            | Risk::WindowsAlternateStream
            | Risk::WindowsForbiddenChar(_)
            | Risk::WindowsControlChar
            | Risk::WindowsTrailingDotOrSpace
            | Risk::WindowsReservedName(_)
            | Risk::WindowsComponentTooLong { .. } => windows_only(os),
            Risk::CaseCollision { .. } => {
                if os.folds_case() {
                    Severity::Blocks
                } else {
                    Severity::ElsewhereOnly(TargetOs::MacOs)
                }
            }
            Risk::NormalizationCollision { .. } => {
                if os.folds_normalization() {
                    Severity::Blocks
                } else {
                    Severity::ElsewhereOnly(TargetOs::MacOs)
                }
            }
            Risk::UnverifiedNormalization => {
                if os.folds_normalization() {
                    Severity::Unverified
                } else {
                    Severity::ElsewhereOnly(TargetOs::MacOs)
                }
            }
            Risk::Probe(ProbeFinding::Failed(_)) => Severity::Unverified,
            Risk::Probe(ProbeFinding::CaseVariantOnDisk { .. }) => {
                if os.folds_case() {
                    Severity::Blocks
                } else {
                    Severity::ElsewhereOnly(TargetOs::MacOs)
                }
            }
            Risk::Probe(_) => Severity::Blocks,
        }
    }

    fn why(&self) -> String {
        match self {
            Risk::PolicyError(said) => format!("정책이 이 경로를 해석하지 못했다 — {said}"),
            Risk::PolicyInconsistent => {
                "같은 정책이라는데 목표 세계가 제외 경로를 담았다 — 식별자와 Manifest 가 맞지 않는다"
                    .to_string()
            }
            Risk::InternalStore => "루트 .gil/ 은 내부 저장소다 — 복원 대상이 아니다".to_string(),
            Risk::Nul => "NUL 바이트".to_string(),
            Risk::Backslash => "`\\` — 구분자 혼합".to_string(),
            Risk::EmptyComponent => "빈 조각 — 절대경로·`//`·UNC/device 접두".to_string(),
            Risk::DotComponent => "`.`/`..` 조각".to_string(),
            Risk::ComponentTooLong { bytes } => format!("조각이 {bytes} 바이트 — 255 를 넘는다"),
            Risk::WindowsDrive => "`X:` 드라이브 상대 경로".to_string(),
            Risk::WindowsAlternateStream => "`:` — NTFS 대체 데이터 스트림 문법".to_string(),
            Risk::WindowsForbiddenChar(c) => format!("Windows 금지 문자 {c:?}"),
            Risk::WindowsControlChar => "제어 문자".to_string(),
            Risk::WindowsTrailingDotOrSpace => "조각 끝의 `.`/공백 — Windows 가 잘라 낸다".to_string(),
            Risk::WindowsReservedName(name) => format!("Windows 예약 이름 {name}"),
            Risk::WindowsComponentTooLong { units } => {
                format!("조각이 UTF-16 {units} 단위 — 255 를 넘는다")
            }
            Risk::UnverifiedNormalization => {
                "결합 문자 — 정규화 표 없이 NFC/NFD 동등성을 판단하지 못한다".to_string()
            }
            Risk::CaseCollision { with } => format!("대소문자만 다른 경로와 겹친다: {}", with.join(", ")),
            Risk::NormalizationCollision { with } => {
                format!("Unicode 정규화만 다른 경로와 겹친다: {}", with.join(", "))
            }
            Risk::TopologyConflict { with, kind } => match kind {
                Topology::FileWhereDirectoryNeeded => {
                    format!("이 파일 자리를 {with:?} 가 디렉터리로 쓴다")
                }
                Topology::DirectoryWhereFileExists => format!("조상 자리에 파일 {with:?} 가 있다"),
            },
            Risk::Probe(finding) => match finding {
                ProbeFinding::SymlinkAncestor { at } => format!("조상 {at:?} 이 심볼릭 링크다"),
                ProbeFinding::SymlinkLeaf => "이 자리가 심볼릭 링크다".to_string(),
                ProbeFinding::AncestorNotDirectory { at, kind } => {
                    format!("조상 {at:?} 이 디렉터리가 아니다({kind:?})")
                }
                ProbeFinding::LeafNotFile { kind } => format!("이 자리가 일반 파일이 아니다({kind:?})"),
                ProbeFinding::Stale { expected, found } => format!(
                    "관측 뒤에 바뀌었다 — Manifest 는 {expected:?}, 디스크는 {found:?}. 이 미리보기는 낡았다"
                ),
                ProbeFinding::CaseVariantOnDisk { name } => {
                    format!("같은 폴더에 대소문자만 다른 {name:?} 이 있다")
                }
                ProbeFinding::Failed(error) => error.to_string(),
            },
        }
    }
}

/// 한 경로의 미리보기.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) path: EntryPath,
    pub(crate) disposition: Disposition,
    pub(crate) risks: Vec<Risk>,
}

impl Entry {
    /// 이 OS 에서 복원을 막는 위험이 하나라도 있는가(미확인 포함).
    pub(crate) fn blocked(&self, os: TargetOs) -> bool {
        self.risks
            .iter()
            .any(|risk| matches!(risk.severity(os), Severity::Blocks | Severity::Unverified))
    }
}

/// 디스크 조회가 어떻게 끝났는가.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeStatus {
    /// 조회했다 — 결과는 각 경로의 `Risk::Probe` 에 있다.
    Checked { adapter: &'static str },
    /// 조회하지 못했다 — **apply 후보 판정을 거절하는 이유다.**
    Unsupported { adapter: &'static str, why: String },
}

/// 전체 판정을 막는 이유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Block {
    PolicyUnknown { before_known: bool, target_known: bool },
    PolicyMismatch { before: String, target: String },
    FileSystemNotProbed { why: String },
    Path { path: String, risk: Risk },
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Block::PolicyUnknown { before_known, target_known } => write!(
                f,
                "정책 식별자 미상 (현재: {}, 목표: {}) — legacy Manifest 에 지금 정책을 소급하지 않는다",
                known(*before_known),
                known(*target_known)
            ),
            Block::PolicyMismatch { before, target } => {
                write!(f, "정책 식별자 불일치 (현재 {before:?}, 목표 {target:?})")
            }
            Block::FileSystemNotProbed { why } => write!(f, "파일 시스템을 조회하지 않았다 — {why}"),
            Block::Path { path, risk } => write!(f, "{path:?}: {}", risk.why()),
        }
    }
}

fn known(flag: bool) -> &'static str {
    if flag { "있음" } else { "없음" }
}

/// 전체 판정.
///
/// `Candidate` 는 「이 미리보기가 아는 한 막는 것이 없다」는 뜻이다. **복원 허용이 아니다** —
/// 제품 복원은 별도 승인·연결·`product-safety` 해제가 있어야 하고, 그 어느 것도 여기 없다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Readiness {
    Candidate,
    Refused(Vec<Block>),
}

/// 미리보기 결과 전부 — **읽기 전용 값이다.** [`Plan`](super::Plan) 으로 가는 문이 없다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Preview {
    os: TargetOs,
    policy: PolicyCheck,
    entries: Vec<Entry>,
    probe: ProbeStatus,
    before_digest: [u8; 32],
    fingerprint: [u8; 32],
}

impl Preview {
    pub(crate) fn os(&self) -> TargetOs {
        self.os
    }

    pub(crate) fn policy(&self) -> &PolicyCheck {
        &self.policy
    }

    /// 경로 UTF-8 바이트 오름차순 — 같은 입력이면 언제나 같은 차례다.
    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub(crate) fn probe(&self) -> &ProbeStatus {
        &self.probe
    }

    /// 입력 전부(두 Manifest·정책 식별자·OS·callback 답)의 지문 — 두 미리보기가 같은 것을 봤는가.
    pub(crate) fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }

    /// 지금 다시 관측한 세계가 **이 미리보기가 본 현재와 같은가.**
    ///
    /// 다르면 이 미리보기는 낡았다 — 결과를 다시 쓰지 말고 다시 만든다. 적용 직전 재관측과
    /// 이 비교가 「관측과 적용 사이의 변동」을 잡는 길이며, 글자 검사가 그것을 대신하지 않는다.
    pub(crate) fn still_matches(&self, now: &Manifest) -> bool {
        manifest_digest(now) == self.before_digest
    }

    pub(crate) fn readiness(&self) -> Readiness {
        let mut blocks = Vec::new();
        match &self.policy {
            PolicyCheck::Matched { .. } => {}
            PolicyCheck::Unknown { before_known, target_known } => blocks.push(Block::PolicyUnknown {
                before_known: *before_known,
                target_known: *target_known,
            }),
            PolicyCheck::Mismatch { before, target } => blocks.push(Block::PolicyMismatch {
                before: before.clone(),
                target: target.clone(),
            }),
        }
        if let ProbeStatus::Unsupported { why, .. } = &self.probe {
            blocks.push(Block::FileSystemNotProbed { why: why.clone() });
        }
        for entry in &self.entries {
            for risk in &entry.risks {
                if matches!(risk.severity(self.os), Severity::Blocks | Severity::Unverified) {
                    blocks.push(Block::Path {
                        path: entry.path.as_str().to_string(),
                        risk: risk.clone(),
                    });
                }
            }
        }
        if blocks.is_empty() {
            Readiness::Candidate
        } else {
            Readiness::Refused(blocks)
        }
    }

    /// 역할별 수.
    pub(crate) fn tally(&self) -> Tally {
        let mut tally = Tally::default();
        for entry in &self.entries {
            match &entry.disposition {
                Disposition::Unchanged => tally.unchanged += 1,
                Disposition::WouldCreate => tally.would_create += 1,
                Disposition::WouldReplace => tally.would_replace += 1,
                Disposition::CurrentOnly { .. } => tally.current_only += 1,
                Disposition::Excluded { .. } => tally.excluded += 1,
            }
            if entry.blocked(self.os) {
                tally.blocked += 1;
            }
        }
        tally
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Tally {
    pub(crate) unchanged: usize,
    pub(crate) would_create: usize,
    pub(crate) would_replace: usize,
    pub(crate) current_only: usize,
    pub(crate) excluded: usize,
    pub(crate) blocked: usize,
}

/// 사람이 읽는 꼴. 복원 명령을 적지 않고, 내부 digest 를 내보이지 않는다.
impl fmt::Display for Preview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "복원 미리보기 — 파일은 하나도 바꾸지 않았다.\n")?;
        writeln!(f, "대상 OS 규칙\n  {}\n", self.os)?;
        match &self.policy {
            PolicyCheck::Matched { id } => writeln!(f, "정책\n  일치 ({id})\n")?,
            PolicyCheck::Unknown { before_known, target_known } => writeln!(
                f,
                "정책\n  미상 — 현재 {}, 목표 {} (legacy 호환 문제)\n",
                known(*before_known),
                known(*target_known)
            )?,
            PolicyCheck::Mismatch { before, target } => {
                writeln!(f, "정책\n  불일치 — 현재 {before:?}, 목표 {target:?}\n")?
            }
        }
        match &self.probe {
            ProbeStatus::Checked { adapter } => {
                writeln!(f, "파일 시스템 조회\n  {adapter} — 조회 시점의 사실이며 적용 시점을 보장하지 않는다\n")?
            }
            ProbeStatus::Unsupported { adapter, why } => {
                writeln!(f, "파일 시스템 조회\n  하지 않음 ({adapter}) — {why}\n")?
            }
        }
        let tally = self.tally();
        writeln!(
            f,
            "요약\n  유지 {}  생성 예정 {}  변경 예정 {}  현재에만 있음(보존) {}  제외 {}  막힘 {}\n",
            tally.unchanged,
            tally.would_create,
            tally.would_replace,
            tally.current_only,
            tally.excluded,
            tally.blocked
        )?;
        writeln!(f, "경로")?;
        for entry in &self.entries {
            if entry.disposition == Disposition::Unchanged && entry.risks.is_empty() {
                continue;
            }
            writeln!(f, "  {}  {}", entry.path, entry.disposition)?;
            for risk in &entry.risks {
                let label = match risk.severity(self.os) {
                    Severity::Blocks => "막힘".to_string(),
                    Severity::Unverified => "미확인".to_string(),
                    Severity::ElsewhereOnly(os) => format!("{os} 에서만 막힘"),
                };
                let basis = match risk.basis() {
                    Basis::Manifest => "Manifest",
                    Basis::FileSystem => "디스크 조회",
                };
                writeln!(f, "    [{label} · {basis}] {}", risk.why())?;
            }
        }
        writeln!(f)?;
        match self.readiness() {
            Readiness::Candidate => {
                write!(
                    f,
                    "판정\n  이 미리보기가 아는 한 막는 것이 없다. 복원 허용이 아니다 — 제품 복원은 여전히 별도 승인과 연결이 필요하다."
                )
            }
            Readiness::Refused(blocks) => {
                writeln!(f, "판정\n  apply 후보가 아니다 — 정보성 diff 로만 쓴다.")?;
                for block in blocks {
                    writeln!(f, "  - {block}")?;
                }
                Ok(())
            }
        }
    }
}

// ── 미리보기 만들기 ─────────────────────────────────────────────────────────

/// 두 세계와 정책을 받아 **파일을 바꾸지 않고** 미리보기를 만든다.
///
/// `includes` 는 `/` 구분 프로젝트 상대 경로 하나를 받아 `Ok(true)` 추적 후보, `Ok(false)` 정책
/// 제외, `Err` 해석 거절을 돌려준다(INTERFACES). `.git` 과 루트 `.gil` 은 callback 에 묻지 않는다.
///
/// `probe` 는 읽기 전용 디스크 조회다. [`NoProbe`] 를 주면 글자 검사만 하고 결과는 명시적으로
/// 「조회하지 않았다」가 되어 apply 후보가 될 수 없다.
pub(crate) fn preview(
    request: &Request<'_>,
    includes: impl Fn(&str) -> Result<bool, String>,
    probe: &dyn Probe,
) -> Preview {
    let os = request.os;
    let policy = check_policy(request.before_policy, request.target_policy);

    // ① 경로의 합집합을 canonical 차례로 — Manifest 가 이미 정렬돼 있지만 두 벌을 합치므로
    //    다시 세운다. 같은 입력은 언제나 같은 차례다.
    let mut paths: Vec<&EntryPath> = request.target.paths().chain(request.before.paths()).collect();
    paths.sort_by(|a, b| a.key().cmp(b.key()));
    paths.dedup();

    // ② 역할과 정책.
    let mut answers: BTreeMap<&str, Result<bool, String>> = BTreeMap::new();
    let mut entries: Vec<Entry> = Vec::with_capacity(paths.len());
    for path in paths {
        let text = path.as_str();
        let before = request.before.get(path);
        let target = request.target.get(path);
        let side = match (before.is_some(), target.is_some()) {
            (true, true) => Side::Both,
            (true, false) => Side::BeforeOnly,
            (false, true) => Side::TargetOnly,
            (false, false) => unreachable!("합집합에서 온 경로다"),
        };
        let mut risks = Vec::new();

        let disposition = if is_internal_store(text) {
            risks.push(Risk::InternalStore);
            diff_role(before, target)
        } else if is_git(text) {
            Disposition::Excluded {
                by: Exclusion::GitAlways,
                side,
            }
        } else {
            let answer = includes(text);
            answers.insert(text, answer.clone());
            match answer {
                Ok(true) => diff_role(before, target),
                Ok(false) => {
                    // 정책이 같다면 목표 세계에 제외 경로가 있을 수 없다. 있다면 식별자가
                    // 거짓이거나 Manifest 가 다른 정책으로 만들어졌을 것이다 — 조용히 넘기지 않는다.
                    if target.is_some() && matches!(policy, PolicyCheck::Matched { .. }) {
                        risks.push(Risk::PolicyInconsistent);
                    }
                    Disposition::Excluded {
                        by: Exclusion::Policy,
                        side,
                    }
                }
                Err(said) => {
                    risks.push(Risk::PolicyError(said));
                    diff_role(before, target)
                }
            }
        };

        risks.extend(inspect_text(text));
        entries.push(Entry {
            path: path.clone(),
            disposition,
            risks,
        });
    }

    // ③ 결과 세계의 겹침 — 목표 ∪ 현재. 현재에만 있는 것은 (제외든 보존이든) 디스크에 남으므로
    //    목표가 만들 자리와 겹칠 수 있다.
    add_collisions(&mut entries, os);

    // ④ 디스크 조회 — 읽기만.
    let probe_status = add_probe_findings(&mut entries, request, probe);

    let before_digest = manifest_digest(request.before);
    let fingerprint = fingerprint(request, &answers, &policy);

    Preview {
        os,
        policy,
        entries,
        probe: probe_status,
        before_digest,
        fingerprint,
    }
}

fn diff_role(before: Option<&ContentDigest>, target: Option<&ContentDigest>) -> Disposition {
    match (before, target) {
        (Some(now), Some(want)) if now == want => Disposition::Unchanged,
        (Some(_), Some(_)) => Disposition::WouldReplace,
        (None, Some(_)) => Disposition::WouldCreate,
        (Some(_), None) => Disposition::CurrentOnly {
            proposal: Preservation::QuarantineProposed,
        },
        (None, None) => unreachable!("합집합에서 온 경로다"),
    }
}

fn check_policy(before: Option<&str>, target: Option<&str>) -> PolicyCheck {
    match (before, target) {
        (Some(b), Some(t)) if b == t => PolicyCheck::Matched { id: b.to_string() },
        (Some(b), Some(t)) => PolicyCheck::Mismatch {
            before: b.to_string(),
            target: t.to_string(),
        },
        (b, t) => PolicyCheck::Unknown {
            before_known: b.is_some(),
            target_known: t.is_some(),
        },
    }
}

fn is_git(text: &str) -> bool {
    text == ".git" || text.starts_with(".git/")
}

fn is_internal_store(text: &str) -> bool {
    text == crate::artifact::GIL_DIR || text.starts_with(".gil/")
}

// ── 글자 검사 ──────────────────────────────────────────────────────────────

/// 경로 **글자**만 보고 아는 위험 — OS 와 무관하게 전부 모은다. 무게는 [`Risk::severity`] 가 OS 별로 정한다.
///
/// `EntryPath` 가 이미 막은 것(빈 조각·`.`·`..`·`\`)도 다시 본다 — 미리보기는 사람이 적은
/// 글자나 다른 저장소에서 온 글자도 받을 수 있어야 하고, 한 검사가 모든 입구를 지켜야 한다.
pub(crate) fn inspect_text(text: &str) -> Vec<Risk> {
    let mut risks = Vec::new();
    if text.contains('\0') {
        risks.push(Risk::Nul);
    }
    if text.contains('\\') {
        risks.push(Risk::Backslash);
    }
    let components: Vec<&str> = text.split('/').collect();
    if components.iter().any(|part| part.is_empty()) {
        risks.push(Risk::EmptyComponent);
    }
    if components.iter().any(|part| *part == "." || *part == "..") {
        risks.push(Risk::DotComponent);
    }
    if let Some(longest) = components.iter().map(|part| part.len()).max()
        && longest > 255
    {
        risks.push(Risk::ComponentTooLong { bytes: longest });
    }

    // Windows.
    if let Some(first) = components.first()
        && first.len() >= 2
        && first.as_bytes()[0].is_ascii_alphabetic()
        && first.as_bytes()[1] == b':'
    {
        risks.push(Risk::WindowsDrive);
    }
    if text.contains(':') {
        risks.push(Risk::WindowsAlternateStream);
    }
    let mut forbidden: Vec<char> = text.chars().filter(|c| "<>\"|?*".contains(*c)).collect();
    forbidden.sort_unstable();
    forbidden.dedup();
    for c in forbidden {
        risks.push(Risk::WindowsForbiddenChar(c));
    }
    if text.chars().any(|c| ('\u{1}'..='\u{1f}').contains(&c)) {
        risks.push(Risk::WindowsControlChar);
    }
    if components
        .iter()
        .any(|part| !part.is_empty() && (part.ends_with('.') || part.ends_with(' ')))
    {
        risks.push(Risk::WindowsTrailingDotOrSpace);
    }
    let mut reserved: Vec<String> = components.iter().filter_map(|part| windows_reserved(part)).collect();
    reserved.sort();
    reserved.dedup();
    for name in reserved {
        risks.push(Risk::WindowsReservedName(name));
    }
    if let Some(longest) = components
        .iter()
        .map(|part| part.encode_utf16().count())
        .max()
        && longest > 255
    {
        risks.push(Risk::WindowsComponentTooLong { units: longest });
    }

    // Unicode — 한글은 알고리즘으로 정규화하므로 여기 들지 않는다.
    if text.chars().any(is_combining_mark) {
        risks.push(Risk::UnverifiedNormalization);
    }
    risks
}

/// Windows 예약 장치 이름 — 확장자와 끝 공백/점을 떼고, 대소문자 없이 본다.
fn windows_reserved(component: &str) -> Option<String> {
    let stem = component.split('.').next().unwrap_or("").trim_end_matches([' ', '.']);
    if stem.is_empty() {
        return None;
    }
    let upper: String = stem.chars().map(|c| c.to_ascii_uppercase()).collect();
    let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (upper.len() >= 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper[3..].chars().count() == 1
            && upper[3..]
                .chars()
                .next()
                .is_some_and(|c| matches!(c, '1'..='9' | '¹' | '²' | '³')));
    reserved.then_some(upper)
}

fn is_combining_mark(c: char) -> bool {
    matches!(
        c as u32,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F
    )
}

// ── 겹침 ───────────────────────────────────────────────────────────────────

/// 결과 세계(목표 ∪ 현재)에서 서로 겹치는 자리를 찾아 양쪽에 적는다.
///
/// 대소문자 접기는 `str::to_lowercase` 근사다 — NTFS upcase 표·APFS 의 접기와 완전히 같지
/// 않다. 그래서 글자 겹침은 「없다」를 증명하지 못하고, 있는 것을 **찾아내는** 쪽으로만 쓴다.
/// 한글은 음절↔자모를 알고리즘으로 풀어 비교하고, 그 밖의 결합 문자는 [`Risk::UnverifiedNormalization`]
/// 으로 남긴다.
fn add_collisions(entries: &mut [Entry], os: TargetOs) {
    // 겹침 검사에는 모든 OS 의 접기를 다 돌린다 — 다른 OS 에서만 막히는 것도 적어야 한다.
    let folded_key = |text: &str| hangul_decomposed(&text.to_lowercase());
    let exact_key = |text: &str| text.to_string();

    // 파일 자리와 디렉터리 자리 — 접은 키로.
    let mut files_by_key: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut dirs_by_key: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut files_exact: BTreeMap<String, String> = BTreeMap::new();
    let mut dirs_exact: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in entries.iter() {
        let text = entry.path.as_str();
        files_by_key.entry(folded_key(text)).or_default().push(text.to_string());
        files_exact.insert(exact_key(text), text.to_string());
        for prefix in prefixes(text) {
            dirs_by_key.entry(folded_key(prefix)).or_default().push(text.to_string());
            dirs_exact.entry(exact_key(prefix)).or_default().push(text.to_string());
        }
    }

    for entry in entries.iter_mut() {
        let text = entry.path.as_str();
        let key = folded_key(text);

        // 대소문자/정규화 겹침 — 같은 접은 키의 **다른** 글자.
        if let Some(group) = files_by_key.get(&key) {
            let mut case_with: Vec<String> = Vec::new();
            let mut norm_with: Vec<String> = Vec::new();
            for other in group {
                if other == text {
                    continue;
                }
                // 정규화만 다른가(대소문자 접기 전 비교), 아니면 대소문자(도) 다른가.
                if hangul_decomposed(other) == hangul_decomposed(text) {
                    norm_with.push(other.clone());
                } else {
                    case_with.push(other.clone());
                }
            }
            if !norm_with.is_empty() {
                norm_with.sort();
                entry.risks.push(Risk::NormalizationCollision { with: norm_with });
            }
            if !case_with.is_empty() {
                case_with.sort();
                entry.risks.push(Risk::CaseCollision { with: case_with });
            }
        }

        // 파일 자리를 다른 경로가 디렉터리로 쓴다.
        let needing: Option<&Vec<String>> = if os.folds_case() {
            dirs_by_key.get(&key)
        } else {
            dirs_exact.get(text)
        };
        if let Some(needing) = needing
            && let Some(with) = needing.iter().min()
        {
            entry.risks.push(Risk::TopologyConflict {
                with: with.clone(),
                kind: Topology::FileWhereDirectoryNeeded,
            });
        }

        // 조상 자리에 파일이 있다.
        let mut blocking: Vec<String> = Vec::new();
        for prefix in prefixes(text) {
            if os.folds_case() {
                if let Some(files) = files_by_key.get(&folded_key(prefix)) {
                    blocking.extend(files.iter().cloned());
                }
            } else if let Some(file) = files_exact.get(prefix) {
                blocking.push(file.clone());
            }
        }
        if let Some(with) = blocking.into_iter().min() {
            entry.risks.push(Risk::TopologyConflict {
                with,
                kind: Topology::DirectoryWhereFileExists,
            });
        }
    }
}

/// `a/b/c.txt` 의 디렉터리 조상 `a`, `a/b`.
fn prefixes(text: &str) -> impl Iterator<Item = &str> {
    text.match_indices('/').map(move |(at, _)| &text[..at])
}

/// 한글 음절(U+AC00–U+D7A3)을 조합형 자모로 풀어 쓴다 — 표 없이 되는 유일한 정규화다.
///
/// NFD 의 한글 부분과 같다. 그 밖의 글자는 그대로 둔다 — 그래서 라틴 결합 문자의 동등성은 이
/// 함수가 모르고, [`is_combining_mark`] 가 「미확인」으로 넘긴다.
fn hangul_decomposed(text: &str) -> String {
    const BASE: u32 = 0xAC00;
    const LAST: u32 = 0xD7A3;
    const L_BASE: u32 = 0x1100;
    const V_BASE: u32 = 0x1161;
    const T_BASE: u32 = 0x11A7;
    const V_COUNT: u32 = 21;
    const T_COUNT: u32 = 28;
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let code = c as u32;
        if (BASE..=LAST).contains(&code) {
            let index = code - BASE;
            let l = L_BASE + index / (V_COUNT * T_COUNT);
            let v = V_BASE + (index % (V_COUNT * T_COUNT)) / T_COUNT;
            let t = index % T_COUNT;
            out.push(char::from_u32(l).expect("초성 범위"));
            out.push(char::from_u32(v).expect("중성 범위"));
            if t != 0 {
                out.push(char::from_u32(T_BASE + t).expect("종성 범위"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ── 디스크 조회 ────────────────────────────────────────────────────────────

/// 각 자리를 읽기 전용으로 들여다보고 본 것을 적는다. **아무것도 바꾸지 않는다.**
fn add_probe_findings(entries: &mut [Entry], request: &Request<'_>, probe: &dyn Probe) -> ProbeStatus {
    // adapter 가 뿌리부터 답하지 못하면 전체를 「조회하지 않았다」로 둔다.
    if let Err(ProbeError::Unsupported(why)) = probe.kind("") {
        return ProbeStatus::Unsupported {
            adapter: probe.describe(),
            why,
        };
    }

    let mut names_cache: BTreeMap<String, Result<Vec<String>, ProbeError>> = BTreeMap::new();

    for entry in entries.iter_mut() {
        let text = entry.path.as_str();
        if matches!(entry.disposition, Disposition::Excluded { .. }) {
            // 제외 자리는 복원이 건드리지 않으므로 디스크를 묻지 않는다 — 다만 겹침은 위에서 이미 봤다.
            continue;
        }

        // 조상.
        let mut ancestor_broken = false;
        for prefix in prefixes(text) {
            match probe.kind(prefix) {
                Ok(EntryKind::Directory) => {}
                Ok(EntryKind::Missing) => break, // 아래도 전부 없다 — 생성 예정이면 정상이다.
                Ok(EntryKind::Symlink) => {
                    entry.risks.push(Risk::Probe(ProbeFinding::SymlinkAncestor {
                        at: prefix.to_string(),
                    }));
                    ancestor_broken = true;
                    break;
                }
                Ok(kind) => {
                    entry.risks.push(Risk::Probe(ProbeFinding::AncestorNotDirectory {
                        at: prefix.to_string(),
                        kind,
                    }));
                    ancestor_broken = true;
                    break;
                }
                Err(error) => {
                    entry.risks.push(Risk::Probe(ProbeFinding::Failed(error)));
                    ancestor_broken = true;
                    break;
                }
            }
        }
        if ancestor_broken {
            continue;
        }

        // 같은 폴더의 이름들 — 잎을 보기 **전에** 본다. 대소문자를 접는 파일 시스템에서는
        // `lstat("dup.txt")` 가 `Dup.txt` 에 닿으므로, 그 적중을 「낡았다」로 잘못 읽지 않으려면
        // 글자 그대로의 이름이 목록에 있는지를 먼저 알아야 한다.
        let (parent, leaf) = match text.rsplit_once('/') {
            Some((parent, leaf)) => (parent.to_string(), leaf),
            None => (String::new(), text),
        };
        let listing = names_cache
            .entry(parent.clone())
            .or_insert_with(|| probe.names(&parent));
        let mut literal_present = None;
        match listing {
            Ok(names) => {
                literal_present = Some(names.iter().any(|name| name == leaf));
                let folded = leaf.to_lowercase();
                for name in names {
                    if name != leaf && name.to_lowercase() == folded {
                        entry.risks.push(Risk::Probe(ProbeFinding::CaseVariantOnDisk {
                            name: name.clone(),
                        }));
                    }
                }
            }
            Err(error) => entry.risks.push(Risk::Probe(ProbeFinding::Failed(error.clone()))),
        }

        // 잎 — `before` 가 본 것과 같은가.
        let expected = if request.before.get(&entry.path).is_some() {
            EntryKind::File
        } else {
            EntryKind::Missing
        };
        let found = match literal_present {
            // 글자 그대로의 이름이 폴더에 없다 — `lstat` 이 무엇에 닿든 그것은 변형이다.
            Some(false) => Ok(EntryKind::Missing),
            _ => probe.kind(text),
        };
        match found {
            Ok(found) if found == expected => {}
            Ok(EntryKind::Symlink) => entry.risks.push(Risk::Probe(ProbeFinding::SymlinkLeaf)),
            Ok(found @ (EntryKind::Directory | EntryKind::Other)) => {
                entry.risks.push(Risk::Probe(ProbeFinding::LeafNotFile { kind: found }))
            }
            Ok(found) => entry.risks.push(Risk::Probe(ProbeFinding::Stale { expected, found })),
            Err(error) => entry.risks.push(Risk::Probe(ProbeFinding::Failed(error))),
        }
    }

    ProbeStatus::Checked {
        adapter: probe.describe(),
    }
}

// ── 지문 ───────────────────────────────────────────────────────────────────

/// Manifest 의 지문 — canonical 차례의 (경로 길이·경로·내용 지문).
fn manifest_digest(manifest: &Manifest) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"GIL-PREVIEW-MANIFEST\0");
    for entry in manifest.entries() {
        let path = entry.path.as_str().as_bytes();
        hasher.update((path.len() as u32).to_be_bytes());
        hasher.update(path);
        hasher.update(crate::artifact::digest_bytes(&entry.content));
    }
    hasher.finalize().into()
}

/// 입력 전부의 지문 — 두 Manifest, 정책 식별자, OS, 그리고 callback 이 각 경로에 한 답.
fn fingerprint(
    request: &Request<'_>,
    answers: &BTreeMap<&str, Result<bool, String>>,
    policy: &PolicyCheck,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"GIL-PREVIEW-INPUT\0");
    hasher.update(manifest_digest(request.before));
    hasher.update(manifest_digest(request.target));
    hasher.update([request.os as u8]);
    let mut tag = |label: &[u8], text: Option<&str>| {
        hasher.update(label);
        match text {
            Some(text) => {
                hasher.update([1u8]);
                hasher.update((text.len() as u32).to_be_bytes());
                hasher.update(text.as_bytes());
            }
            None => hasher.update([0u8]),
        }
    };
    tag(b"before-policy", request.before_policy);
    tag(b"target-policy", request.target_policy);
    let policy_text = format!("{policy:?}");
    hasher.update((policy_text.len() as u32).to_be_bytes());
    hasher.update(policy_text.as_bytes());
    for (path, answer) in answers {
        hasher.update((path.len() as u32).to_be_bytes());
        hasher.update(path.as_bytes());
        match answer {
            Ok(true) => hasher.update([1u8]),
            Ok(false) => hasher.update([0u8]),
            Err(said) => {
                hasher.update([2u8]);
                hasher.update((said.len() as u32).to_be_bytes());
                hasher.update(said.as_bytes());
            }
        }
    }
    hasher.finalize().into()
}
