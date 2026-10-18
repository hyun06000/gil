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
