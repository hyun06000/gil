//! 시각 Report 계약과 실험적 format-5 저장/Session 연결.
//!
//! 이 모듈의 선언 검증은 화면 관측의 증거가 아니다. 이미지 디코딩은 image 하위 모듈,
//! 저장 검증은 registry/project가 담당한다. 사용자 MCP 도구 및 자동 이관은 아직 없다.

use std::collections::BTreeSet;
use std::num::NonZeroU64;

pub mod project;
pub mod display;
pub mod registry;
pub mod image;
pub(crate) mod file;
#[cfg(any(unix, windows))]
pub(crate) mod state_file;

/// Project 안의 논리 ID. digest·파일 경로·접근 권한이 아니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VisualAssetId(NonZeroU64);

impl VisualAssetId {
    pub fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
}

/// 보고서별 참조 설명. 원래 자료의 bytes·생성 출처를 덮어쓰는 필드는 없다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualUse {
    pub asset: VisualAssetId,
    pub caption: String,
    pub alt: String,
    pub purpose: String,
}

/// 새 Verify의 선언. 구형 기록의 미기재는 이 enum의 값이 아니다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualDeclaration {
    Attached {
        uses: Vec<VisualUse>,
    },
    NotNeeded {
        reason: String,
    },
    /// 부분 첨부도 허용한다. 없는 자료를 불필요하다고 바꾸지 않는다.
    Missing {
        reason: String,
        follow_up: String,
        uses: Vec<VisualUse>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualValidationError {
    DeclarationRequired,
    EmptyAttachments,
    BlankReason,
    /// 이유가 상투어이거나 한 단어다. 무엇이 이미지를 대신하는 증거인지 말해야 한다.
    WeakReason,
    BlankFollowUp,
    BlankDescription {
        index: usize,
        field: VisualDescriptionField,
    },
    DuplicateAsset {
        asset: VisualAssetId,
    },
    UnavailableAsset {
        asset: VisualAssetId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualDescriptionField {
    Caption,
    Alt,
    Purpose,
}

impl VisualDeclaration {
    pub fn uses(&self) -> &[VisualUse] {
        match self {
            Self::Attached { uses } | Self::Missing { uses, .. } => uses,
            Self::NotNeeded { .. } => &[],
        }
    }

    /// **새 쓰기에만** 적용하는 이유의 실질 검사. 저장된 기록을 읽을 때는 부르지 않는다 —
    /// 이전 판이 받아들인 "필요 없음" 은 그대로 읽혀야 한다(호환성).
    pub fn check_reason_substance(&self) -> Result<(), VisualValidationError> {
        match self {
            Self::Attached { .. } => Ok(()),
            Self::NotNeeded { reason } | Self::Missing { reason, .. } => require_reason(reason),
        }
    }

    /// 승인되지 않은 파일/hash를 여기서 찾지 않는다. 호출자는 현재 Project에서 확정된 자료와
    /// 같은 Verify close에 원자적으로 포함될 자료만 허용하는 resolver를 제공해야 한다.
    /// 이 함수는 resolver의 권한 검증을 대신하지 않는다.
    pub fn validate(
        &self,
        available: impl FnMut(VisualAssetId) -> bool,
    ) -> Result<(), VisualValidationError> {
        match self {
            Self::Attached { uses } => {
                if uses.is_empty() {
                    return Err(VisualValidationError::EmptyAttachments);
                }
                validate_uses(uses, available)
            }
            Self::NotNeeded { reason } => require_text(reason, VisualValidationError::BlankReason),
            Self::Missing {
                reason,
                follow_up,
                uses,
            } => {
                require_text(reason, VisualValidationError::BlankReason)?;
                require_text(follow_up, VisualValidationError::BlankFollowUp)?;
                validate_uses(uses, available)
            }
        }
    }
}

/// 새 Verify 쓰기 경계의 검사. 구형 Report 읽기에 호출하면 안 된다.
pub fn validate_new_verify(
    declaration: Option<&VisualDeclaration>,
    available: impl FnMut(VisualAssetId) -> bool,
) -> Result<(), VisualValidationError> {
    declaration
        .ok_or(VisualValidationError::DeclarationRequired)?
        .validate(available)
}

/// Step/Cycle의 공통 참조 검사. Verify 외 Report의 참조 목록은 비어도 된다.
/// 순서는 입력 그대로이며 다른 Report에서 같은 자료를 참조하는 것을 막지 않는다.
pub fn validate_uses(
    uses: &[VisualUse],
    mut available: impl FnMut(VisualAssetId) -> bool,
) -> Result<(), VisualValidationError> {
    let mut seen = BTreeSet::new();
    for (index, usage) in uses.iter().enumerate() {
        for (field, text) in [
            (VisualDescriptionField::Caption, &usage.caption),
            (VisualDescriptionField::Alt, &usage.alt),
            (VisualDescriptionField::Purpose, &usage.purpose),
        ] {
            require_text(
                text,
                VisualValidationError::BlankDescription { index, field },
            )?;
        }
        if !seen.insert(usage.asset) {
            return Err(VisualValidationError::DuplicateAsset { asset: usage.asset });
        }
        if !available(usage.asset) {
            return Err(VisualValidationError::UnavailableAsset { asset: usage.asset });
        }
    }
    Ok(())
}

fn require_text(text: &str, error: VisualValidationError) -> Result<(), VisualValidationError> {
    if text.trim().is_empty() {
        Err(error)
    } else {
        Ok(())
    }
}

/// 상투어 목록. 정규화(소문자·공백 하나·끝 구두점 제거) 후 전체 일치만 거절한다.
const BOILERPLATE_REASONS: &[&str] = &[
    "not needed", "not required", "not applicable", "no image", "no images", "no visual",
    "no visuals", "none", "n/a", "na", "text only", "no", "없음", "불필요", "필요 없음",
    "필요없음", "해당 없음", "해당없음", "이미지 없음", "시각 자료 없음", "시각자료 없음",
    "텍스트만", "필요하지 않음",
];

/// `not_needed`/`missing` 의 이유가 **무엇이 이미지를 대신하는지** 말하는가.
///
/// 기계적으로 잴 수 있는 것만 잰다: 비어 있지 않고, 두 단어 이상이며, 여섯 글자 이상이고,
/// 상투어 하나로 끝나지 않는다. 내용의 진실성은 여기서 판정하지 않는다.
pub fn reason_is_substantive(reason: &str) -> bool {
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        return false;
    }
    let words = trimmed.split_whitespace().count();
    let letters = trimmed.chars().filter(|c| !c.is_whitespace()).count();
    if words < 2 || letters < 6 {
        return false;
    }
    let normalized = trimmed
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let normalized = normalized.trim_end_matches(['.', '!', '。', '…']);
    !BOILERPLATE_REASONS.contains(&normalized)
}

/// `evidence_files` 본문에서 첫 이미지 파일 경로. 확장자만 본다 — 내용은 첨부 경계가 검사한다.
pub fn image_evidence_path(evidence_files: &str) -> Option<String> {
    const IMAGE: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "tif", "tiff"];
    evidence_files
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find(|line| {
            std::path::Path::new(line)
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| IMAGE.contains(&ext.to_ascii_lowercase().as_str()))
        })
        .map(str::to_string)
}

fn require_reason(reason: &str) -> Result<(), VisualValidationError> {
    require_text(reason, VisualValidationError::BlankReason)?;
    if reason_is_substantive(reason) {
        Ok(())
    } else {
        Err(VisualValidationError::WeakReason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(id: u64) -> VisualUse {
        VisualUse {
            asset: VisualAssetId::new(NonZeroU64::new(id).unwrap()),
            caption: "계산기 화면".into(),
            alt: "숫자와 연산 버튼".into(),
            purpose: "버튼 배치 확인".into(),
        }
    }

    #[test]
    fn new_verify_cannot_silently_omit_declaration() {
        assert_eq!(
            validate_new_verify(None, |_| true),
            Err(VisualValidationError::DeclarationRequired)
        );
    }

    #[test]
    fn attached_requires_at_least_one_asset() {
        assert_eq!(
            VisualDeclaration::Attached { uses: vec![] }.validate(|_| true),
            Err(VisualValidationError::EmptyAttachments)
        );
    }

    #[test]
    fn attached_resolves_every_asset_in_order_without_mutation() {
        let declared = VisualDeclaration::Attached {
            uses: vec![usage(2), usage(1)],
        };
        let before = declared.clone();
        let mut checked = vec![];
        assert_eq!(
            validate_new_verify(Some(&declared), |id| {
                checked.push(id);
                true
            }),
            Ok(())
        );
        assert_eq!(checked, vec![usage(2).asset, usage(1).asset]);
        assert_eq!(declared, before);
    }

    #[test]
    fn not_needed_requires_non_whitespace_reason() {
        for reason in ["", " \n\t", "\u{3000}"] {
            assert_eq!(
                VisualDeclaration::NotNeeded {
                    reason: reason.into()
                }
                .validate(|_| panic!()),
                Err(VisualValidationError::BlankReason)
            );
        }
        assert_eq!(
            VisualDeclaration::NotNeeded {
                reason: "문자열의 완전 일치를 비교했다".into()
            }
            .validate(|_| panic!()),
            Ok(())
        );
    }

    #[test]
    fn not_needed_refuses_boilerplate_and_one_word_reasons() {
        for reason in [
            "없음", "불필요", "필요 없음", "Not needed.", "N/A", "none", "no images",
            "텍스트만", "numbers", "짧음 ok",
        ] {
            let declared = VisualDeclaration::NotNeeded { reason: reason.into() };
            // Stored records stay readable; only the write boundary refuses.
            assert_eq!(declared.validate(|_| panic!()), Ok(()), "{reason:?}");
            assert_eq!(declared.check_reason_substance(), Err(VisualValidationError::WeakReason), "{reason:?}");
        }
        for reason in [
            "numeric assertions only",
            "두 문자열의 바이트 일치만 비교했고 화면이 없다",
            "로그 파일의 종료 코드 0을 확인한 검증이다",
        ] {
            assert!(reason_is_substantive(reason), "{reason:?}");
        }
        // missing 의 이유도 같은 기준이다.
        assert_eq!(
            VisualDeclaration::Missing { reason: "없음".into(), follow_up: "다음 실험".into(), uses: vec![] }
                .check_reason_substance(),
            Err(VisualValidationError::WeakReason)
        );
    }

    #[test]
    fn missing_all_or_some_is_explicit_and_not_a_verdict() {
        for uses in [vec![], vec![usage(1)]] {
            let declaration = VisualDeclaration::Missing {
                reason: "화면 캡처 실패".into(),
                follow_up: "다음 실험에서 재시도".into(),
                uses,
            };
            assert_eq!(declaration.validate(|_| true), Ok(()));
            assert!(matches!(declaration, VisualDeclaration::Missing { .. }));
        }
    }

    #[test]
    fn missing_requires_reason_and_follow_up() {
        for (reason, follow_up, error) in [
            (" ", "다음 실험", VisualValidationError::BlankReason),
            ("누락", "\n", VisualValidationError::BlankFollowUp),
        ] {
            let declaration = VisualDeclaration::Missing {
                reason: reason.into(),
                follow_up: follow_up.into(),
                uses: vec![],
            };
            assert_eq!(declaration.validate(|_| true), Err(error));
        }
    }

    #[test]
    fn image_evidence_is_recognised_by_extension_only() {
        assert_eq!(image_evidence_path("evidence/result.md\n  evidence/plot.PNG \n"), Some("evidence/plot.PNG".into()));
        assert_eq!(image_evidence_path("evidence/result.md\nnotes/png-format.md"), None);
        assert_eq!(image_evidence_path(""), None);
    }

    #[test]
    fn other_reports_can_have_no_visual_references() {
        assert_eq!(validate_uses(&[], |_| panic!()), Ok(()));
    }

    #[test]
    fn unavailable_asset_is_rejected_without_mutating_report() {
        let uses = vec![usage(1), usage(2)];
        let before = uses.clone();
        assert_eq!(
            validate_uses(&uses, |id| id == usage(1).asset),
            Err(VisualValidationError::UnavailableAsset {
                asset: usage(2).asset
            })
        );
        assert_eq!(uses, before);
    }

    #[test]
    fn duplicate_in_one_report_is_rejected() {
        assert_eq!(
            validate_uses(&[usage(1), usage(1)], |_| true),
            Err(VisualValidationError::DuplicateAsset {
                asset: usage(1).asset
            })
        );
    }

    #[test]
    fn same_asset_can_be_referenced_by_multiple_reports_with_different_purposes() {
        let verify = usage(1);
        let mut define = verify.clone();
        define.purpose = "개선 전 화면으로 참조".into();
        for references in [&[verify.clone()][..], &[define.clone()][..]] {
            assert_eq!(validate_uses(references, |id| id == verify.asset), Ok(()));
        }
        assert_eq!(verify.asset, define.asset);
        assert_ne!(verify.purpose, define.purpose);
    }

    #[test]
    fn description_fields_cannot_be_blank() {
        for field in [
            VisualDescriptionField::Caption,
            VisualDescriptionField::Alt,
            VisualDescriptionField::Purpose,
        ] {
            let mut item = usage(1);
            match field {
                VisualDescriptionField::Caption => item.caption = " ".into(),
                VisualDescriptionField::Alt => item.alt = "\t".into(),
                VisualDescriptionField::Purpose => item.purpose = "\n".into(),
            }
            assert_eq!(
                validate_uses(&[item], |_| true),
                Err(VisualValidationError::BlankDescription { index: 0, field })
            );
        }
    }

    #[test]
    fn descriptions_are_not_interpreted_or_sanitized_here() {
        let mut item = usage(1);
        item.caption = "<script>alert(1)</script> file:///private/example #7 3.10".into();
        let before = item.clone();
        assert_eq!(validate_uses(std::slice::from_ref(&item), |_| true), Ok(()));
        assert_eq!(item, before); // renderer must use textContent, never HTML.
    }
}
