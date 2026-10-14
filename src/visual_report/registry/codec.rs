//! Registry의 엄격한 DTO codec. Project format 5에는 같은 DTO를 포함한다.
//! version은 registry DTO의 판이지 Project format이 아니다. 파일 I/O는 store 계층이 맡는다.
//! 이미지 bytes 검증과 사용자 이관 명령은 아직 미구현이다.

use super::*;
use serde::{Deserialize, Serialize};

const VERSION: u32 = 1;
/// 전체 metadata 문서의 임시 방어 한도. 이미지 bytes는 이 문서에 들어가지 않는다.
pub const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosedReportKind {
    Step(NodeKind),
    Cycle,
}

/// 저장된 Graph에서 가져와야 하는 증거. 이 codec은 Project scope나 Graph를 발명하지 않는다.
pub trait VisualGraph {
    fn closed_report_kind(&self, owner: ReportOwner) -> Option<ClosedReportKind>;
    /// 저장 형식/이관 경계에 따라 선언이 필수인 새 Verify들. 구형 미기재 Report는 제외한다.
    fn required_verify_reports(&self) -> Vec<StepRef>;
    /// 원래 Verify가 이 Report close 때 존재했는지 검사한다. 단순 ID 대소 비교는 불충분하다.
    fn reference_was_available(&self, origin: StepRef, consumer: ReportOwner) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    TooLarge,
    Syntax,
    Version(u32),
    InvalidIdSequence,
    InvalidReference,
    InvalidMetadata,
    DuplicateReport,
    MissingClosedReport,
    MissingDeclarationRecord,
    ReportKindMismatch,
    InvalidReport(VisualValidationError),
    InvalidOrigin,
    UnavailableAtClose,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredRegistry {
    version: u32,
    next_id: u64,
    assets: Vec<StoredAsset>,
    reports: Vec<StoredReport>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredAsset {
    id: u64,
    origin: String,
    digest: [u8; 32],
    image_kind: StoredImageKind,
    byte_len: NonZeroU64,
    width: NonZeroU32,
    height: NonZeroU32,
    source_kind: StoredSourceKind,
    source_description: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredImageKind {
    Png,
    Jpeg,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredSourceKind {
    Captured,
    Generated,
    Provided,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredReport {
    owner: String,
    visuals: StoredVisuals,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum StoredVisuals {
    Attached {
        uses: Vec<StoredUse>,
    },
    NotNeeded {
        reason: String,
    },
    Missing {
        reason: String,
        follow_up: String,
        uses: Vec<StoredUse>,
    },
    References {
        uses: Vec<StoredUse>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredUse {
    asset: NonZeroU64,
    caption: String,
    alt: String,
    purpose: String,
}

fn owner_text(owner: ReportOwner) -> String {
    match owner {
        ReportOwner::Step(step) => step.to_string(),
        ReportOwner::Cycle(cycle) => cycle.to_string(),
    }
}

fn parse_owner(text: &str) -> Result<ReportOwner, CodecError> {
    if text.starts_with("step:") {
        text.parse()
            .map(ReportOwner::Step)
            .map_err(|_| CodecError::InvalidReference)
    } else {
        text.parse()
            .map(ReportOwner::Cycle)
            .map_err(|_| CodecError::InvalidReference)
    }
}

fn store_uses(uses: &[VisualUse]) -> Vec<StoredUse> {
    uses.iter()
        .map(|usage| StoredUse {
            asset: usage.asset.0,
            caption: usage.caption.clone(),
            alt: usage.alt.clone(),
            purpose: usage.purpose.clone(),
        })
        .collect()
}

fn read_uses(uses: Vec<StoredUse>) -> Vec<VisualUse> {
    uses.into_iter()
        .map(|usage| VisualUse {
            asset: VisualAssetId::new(usage.asset),
            caption: usage.caption,
            alt: usage.alt,
            purpose: usage.purpose,
        })
        .collect()
}

fn store_visuals(visuals: &ReportVisuals) -> StoredVisuals {
    match visuals {
        ReportVisuals::Verify(VisualDeclaration::Attached { uses }) => StoredVisuals::Attached {
            uses: store_uses(uses),
        },
        ReportVisuals::Verify(VisualDeclaration::NotNeeded { reason }) => {
            StoredVisuals::NotNeeded {
                reason: reason.clone(),
            }
        }
        ReportVisuals::Verify(VisualDeclaration::Missing {
            reason,
            follow_up,
            uses,
        }) => StoredVisuals::Missing {
            reason: reason.clone(),
            follow_up: follow_up.clone(),
            uses: store_uses(uses),
        },
        ReportVisuals::References(uses) => StoredVisuals::References {
            uses: store_uses(uses),
        },
    }
}

fn read_visuals(visuals: StoredVisuals) -> ReportVisuals {
    match visuals {
        StoredVisuals::Attached { uses } => ReportVisuals::Verify(VisualDeclaration::Attached {
            uses: read_uses(uses),
        }),
        StoredVisuals::NotNeeded { reason } => {
            ReportVisuals::Verify(VisualDeclaration::NotNeeded { reason })
        }
        StoredVisuals::Missing {
            reason,
            follow_up,
            uses,
        } => ReportVisuals::Verify(VisualDeclaration::Missing {
            reason,
            follow_up,
            uses: read_uses(uses),
        }),
        StoredVisuals::References { uses } => ReportVisuals::References(read_uses(uses)),
    }
}

fn check_graph(registry: &VisualRegistry, graph: &impl VisualGraph) -> Result<(), CodecError> {
    let mut expected = 1u64;
    for id in registry.assets.keys() {
        if id.0.get() != expected {
            return Err(CodecError::InvalidIdSequence);
        }
        expected = expected
            .checked_add(1)
            .ok_or(CodecError::InvalidIdSequence)?;
    }
    if expected != registry.next_id {
        return Err(CodecError::InvalidIdSequence);
    }
    for step in graph.required_verify_reports() {
        if !matches!(
            registry.reports.get(&ReportOwner::Step(step)),
            Some(ReportVisuals::Verify(_))
        ) {
            return Err(CodecError::MissingDeclarationRecord);
        }
    }
    for (owner, report) in &registry.reports {
        let kind = graph
            .closed_report_kind(*owner)
            .ok_or(CodecError::MissingClosedReport)?;
        let is_verify = match (owner, kind) {
            (
                ReportOwner::Step(_),
                ClosedReportKind::Step(NodeKind::CycleEntry | NodeKind::CycleExit),
            ) => return Err(CodecError::ReportKindMismatch),
            (ReportOwner::Step(_), ClosedReportKind::Step(kind)) => kind == NodeKind::Verify,
            (ReportOwner::Cycle(_), ClosedReportKind::Cycle) => false,
            _ => return Err(CodecError::ReportKindMismatch),
        };
        let available = |id| registry.assets.contains_key(&id);
        match (report, is_verify) {
            (ReportVisuals::Verify(declaration), true) => declaration.validate(available),
            (ReportVisuals::References(uses), false) => validate_uses(uses, available),
            _ => return Err(CodecError::ReportKindMismatch),
        }
        .map_err(CodecError::InvalidReport)?;
        for usage in report.uses() {
            let asset = &registry.assets[&usage.asset];
            if !graph.reference_was_available(asset.origin, *owner) {
                return Err(CodecError::UnavailableAtClose);
            }
        }
    }
    for (id, asset) in &registry.assets {
        if asset.metadata.source_description.trim().is_empty() {
            return Err(CodecError::InvalidMetadata);
        }
        let owner = ReportOwner::Step(asset.origin);
        if graph.closed_report_kind(owner) != Some(ClosedReportKind::Step(NodeKind::Verify)) {
            return Err(CodecError::InvalidOrigin);
        }
        // 생성 Verify가 실제로 자기 Report에 자료를 확정했는지 역방향까지 검사한다.
        if registry.asset_for_report(owner, *id).is_none() {
            return Err(CodecError::InvalidOrigin);
        }
    }
    Ok(())
}

/// 엄격한 DTO 출력. bytes는 포함하지 않는다. 반환값을 별도 sidecar로 쓰면 안 된다.
pub fn encode(registry: &VisualRegistry, graph: &impl VisualGraph) -> Result<Vec<u8>, CodecError> {
    let stored = to_stored(registry, graph)?;
    encode_stored(&stored)
}

pub(crate) fn to_stored(
    registry: &VisualRegistry,
    graph: &impl VisualGraph,
) -> Result<StoredRegistry, CodecError> {
    check_graph(registry, graph)?;
    let stored = StoredRegistry {
        version: VERSION,
        next_id: registry.next_id,
        assets: registry
            .assets
            .iter()
            .map(|(id, asset)| StoredAsset {
                id: id.0.get(),
                origin: asset.origin.to_string(),
                digest: asset.metadata.digest,
                image_kind: match asset.metadata.image_kind {
                    ImageKind::Png => StoredImageKind::Png,
                    ImageKind::Jpeg => StoredImageKind::Jpeg,
                },
                byte_len: asset.metadata.byte_len,
                width: asset.metadata.width,
                height: asset.metadata.height,
                source_kind: match asset.metadata.source_kind {
                    SourceKind::Captured => StoredSourceKind::Captured,
                    SourceKind::Generated => StoredSourceKind::Generated,
                    SourceKind::Provided => StoredSourceKind::Provided,
                },
                source_description: asset.metadata.source_description.clone(),
            })
            .collect(),
        reports: registry
            .reports
            .iter()
            .map(|(owner, report)| StoredReport {
                owner: owner_text(*owner),
                visuals: store_visuals(report),
            })
            .collect(),
    };
    // apply the same bounded output contract when embedding in the Project DTO.
    encode_stored(&stored)?;
    Ok(stored)
}

fn encode_stored(stored: &StoredRegistry) -> Result<Vec<u8>, CodecError> {
    let mut output = LimitedOutput {
        bytes: Vec::new(),
        exceeded: false,
    };
    serde_json::to_writer(&mut output, &stored).map_err(|_| {
        if output.exceeded {
            CodecError::TooLarge
        } else {
            CodecError::Syntax
        }
    })?;
    Ok(output.bytes)
}

struct LimitedOutput {
    bytes: Vec<u8>,
    exceeded: bool,
}

impl std::io::Write for LimitedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_DOCUMENT_BYTES - self.bytes.len() {
            self.exceeded = true;
            return Err(std::io::Error::other("visual metadata exceeds limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// 전체 후보를 검사한 뒤 반환한다. 부분 registry를 호출자에게 노출하지 않는다.
pub fn decode(bytes: &[u8], graph: &impl VisualGraph) -> Result<VisualRegistry, CodecError> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(CodecError::TooLarge);
    }
    #[derive(Deserialize)]
    struct Header {
        version: u32,
    }
    let header: Header = serde_json::from_slice(bytes).map_err(|_| CodecError::Syntax)?;
    if header.version != VERSION {
        return Err(CodecError::Version(header.version));
    }
    let stored: StoredRegistry = serde_json::from_slice(bytes).map_err(|_| CodecError::Syntax)?;
    from_stored(stored, graph)
}

pub(crate) fn from_stored(
    stored: StoredRegistry,
    graph: &impl VisualGraph,
) -> Result<VisualRegistry, CodecError> {
    if stored.version != VERSION {
        return Err(CodecError::Version(stored.version));
    }
    encode_stored(&stored)?;
    let mut registry = VisualRegistry::default();
    for asset in stored.assets {
        if asset.id != registry.next_id {
            return Err(CodecError::InvalidIdSequence);
        }
        let id = NonZeroU64::new(asset.id).ok_or(CodecError::InvalidIdSequence)?;
        registry.next_id = registry
            .next_id
            .checked_add(1)
            .ok_or(CodecError::InvalidIdSequence)?;
        registry.assets.insert(
            VisualAssetId::new(id),
            VisualAsset {
                origin: asset
                    .origin
                    .parse()
                    .map_err(|_| CodecError::InvalidReference)?,
                metadata: NewVisualAsset {
                    digest: asset.digest,
                    image_kind: match asset.image_kind {
                        StoredImageKind::Png => ImageKind::Png,
                        StoredImageKind::Jpeg => ImageKind::Jpeg,
                    },
                    byte_len: asset.byte_len,
                    width: asset.width,
                    height: asset.height,
                    source_kind: match asset.source_kind {
                        StoredSourceKind::Captured => SourceKind::Captured,
                        StoredSourceKind::Generated => SourceKind::Generated,
                        StoredSourceKind::Provided => SourceKind::Provided,
                    },
                    source_description: asset.source_description,
                },
            },
        );
    }
    if registry.next_id != stored.next_id {
        return Err(CodecError::InvalidIdSequence);
    }
    for report in stored.reports {
        let owner = parse_owner(&report.owner)?;
        if registry
            .reports
            .insert(owner, read_visuals(report.visuals))
            .is_some()
        {
            return Err(CodecError::DuplicateReport);
        }
    }
    check_graph(&registry, graph)?;
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    struct Graph {
        reports: BTreeMap<ReportOwner, ClosedReportKind>,
        available: bool,
        required: Vec<StepRef>,
    }
    impl VisualGraph for Graph {
        fn required_verify_reports(&self) -> Vec<StepRef> {
            self.required.clone()
        }
        fn closed_report_kind(&self, owner: ReportOwner) -> Option<ClosedReportKind> {
            self.reports.get(&owner).copied()
        }
        fn reference_was_available(&self, _: StepRef, _: ReportOwner) -> bool {
            self.available
        }
    }
    fn step(cycle: u32, step: u32) -> ReportOwner {
        ReportOwner::Step(StepRef::new(CycleRef::new(cycle).unwrap(), step).unwrap())
    }
    fn usage(id: VisualAssetId) -> VisualUse {
        VisualUse {
            asset: id,
            caption: "그림 #7 3.10\n<script>표시만</script>".into(),
            alt: "화면".into(),
            purpose: "비교".into(),
        }
    }
    fn fixture() -> (VisualRegistry, Graph) {
        let origin = step(1, 3);
        let registry = VisualRegistry::default()
            .prepare_close(
                origin,
                TargetState::OpenStep(NodeKind::Verify),
                vec![NewVisualAsset {
                    digest: [3; 32],
                    image_kind: ImageKind::Png,
                    byte_len: NonZeroU64::new(123).unwrap(),
                    width: NonZeroU32::new(3).unwrap(),
                    height: NonZeroU32::new(4).unwrap(),
                    source_kind: SourceKind::Captured,
                    source_description: "합성 출처".into(),
                }],
                |ids| {
                    ReportVisuals::Verify(VisualDeclaration::Attached {
                        uses: vec![usage(ids[0])],
                    })
                },
            )
            .unwrap();
        let mut graph = Graph {
            reports: [(origin, ClosedReportKind::Step(NodeKind::Verify))].into(),
            available: true,
            required: vec![StepRef::new(CycleRef::new(1).unwrap(), 3).unwrap()],
        };
        let mut registry = registry;
        for (owner, state, kind) in [
            (
                ReportOwner::Cycle(CycleRef::new(1).unwrap()),
                TargetState::OpenCycle,
                ClosedReportKind::Cycle,
            ),
            (
                step(2, 1),
                TargetState::OpenStep(NodeKind::Define),
                ClosedReportKind::Step(NodeKind::Define),
            ),
        ] {
            registry = registry
                .prepare_close(owner, state, vec![], |_| {
                    ReportVisuals::References(vec![usage(VisualAssetId::new(
                        NonZeroU64::new(1).unwrap(),
                    ))])
                })
                .unwrap();
            graph.reports.insert(owner, kind);
        }
        (registry, graph)
    }
    fn document() -> (Value, Graph) {
        let (registry, graph) = fixture();
        (
            serde_json::from_slice(&encode(&registry, &graph).unwrap()).unwrap(),
            graph,
        )
    }
    fn read(value: &Value, graph: &Graph) -> Result<VisualRegistry, CodecError> {
        decode(&serde_json::to_vec(value).unwrap(), graph)
    }

    #[test]
    fn round_trip_keeps_shared_assets_provenance_order_and_text() {
        let (registry, graph) = fixture();
        let bytes = encode(&registry, &graph).unwrap();
        let restored = decode(&bytes, &graph).unwrap();
        assert_eq!(restored, registry);
        assert_eq!(encode(&restored, &graph).unwrap(), bytes);
    }

    #[test]
    fn empty_registry_round_trips_without_fabricating_old_declarations() {
        let (_, mut graph) = fixture(); // legacy reports are explicitly distinguished by the witness.
        graph.required.clear();
        let empty = VisualRegistry::default();
        assert_eq!(
            decode(&encode(&empty, &graph).unwrap(), &graph).unwrap(),
            empty
        );
    }

    #[test]
    fn declaration_variants_and_metadata_variants_round_trip() {
        for declaration in [
            VisualDeclaration::NotNeeded {
                reason: "필요 없음".into(),
            },
            VisualDeclaration::Missing {
                reason: "누락".into(),
                follow_up: "다음 실험".into(),
                uses: vec![],
            },
        ] {
            let (_, graph) = fixture();
            let registry = VisualRegistry::default()
                .prepare_close(
                    step(1, 3),
                    TargetState::OpenStep(NodeKind::Verify),
                    vec![],
                    |_| ReportVisuals::Verify(declaration),
                )
                .unwrap();
            assert_eq!(
                decode(&encode(&registry, &graph).unwrap(), &graph).unwrap(),
                registry
            );
        }
        for image in ["png", "jpeg"] {
            for source in ["captured", "generated", "provided"] {
                let (mut doc, graph) = document();
                doc["assets"][0]["image_kind"] = json!(image);
                doc["assets"][0]["source_kind"] = json!(source);
                let registry = read(&doc, &graph).unwrap();
                assert_eq!(
                    decode(&encode(&registry, &graph).unwrap(), &graph).unwrap(),
                    registry
                );
            }
        }
    }

    #[test]
    fn future_version_is_reported_before_other_shape_checks() {
        let (_, graph) = fixture();
        assert_eq!(
            read(&json!({"version": 99, "future": true}), &graph),
            Err(CodecError::Version(99))
        );
    }

    #[test]
    fn unknown_fields_at_every_level_are_rejected() {
        for path in [
            "",
            "/assets/0",
            "/reports/0",
            "/reports/0/visuals",
            "/reports/0/visuals/uses/0",
        ] {
            let (mut doc, graph) = document();
            doc.pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unexpected".into(), json!(true));
            assert_eq!(read(&doc, &graph), Err(CodecError::Syntax), "{path}");
        }
    }

    #[test]
    fn duplicate_json_fields_and_trailing_data_are_rejected() {
        let (registry, graph) = fixture();
        let text = String::from_utf8(encode(&registry, &graph).unwrap()).unwrap();
        let duplicate = text.replacen("\"version\":1", "\"version\":1,\"version\":1", 1);
        assert_eq!(
            decode(duplicate.as_bytes(), &graph),
            Err(CodecError::Syntax)
        );
        assert_eq!(
            decode(format!("{text} {{}}").as_bytes(), &graph),
            Err(CodecError::Syntax)
        );
    }

    #[test]
    fn every_truncation_of_a_valid_document_is_rejected() {
        let (registry, graph) = fixture();
        let bytes = encode(&registry, &graph).unwrap();
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end], &graph).is_err(), "{end}");
        }
    }

    #[test]
    fn oversized_input_is_rejected_before_json_parse() {
        let (_, graph) = fixture();
        assert_eq!(
            decode(&vec![b' '; MAX_DOCUMENT_BYTES + 1], &graph),
            Err(CodecError::TooLarge)
        );
    }

    #[test]
    fn gaps_zero_duplicate_ids_and_wrong_next_id_are_rejected() {
        for value in [0, 2, u64::MAX] {
            let (mut doc, graph) = document();
            doc["assets"][0]["id"] = json!(value);
            assert_eq!(read(&doc, &graph), Err(CodecError::InvalidIdSequence));
        }
        let (mut doc, graph) = document();
        let duplicate = doc["assets"][0].clone();
        doc["assets"].as_array_mut().unwrap().push(duplicate);
        assert_eq!(read(&doc, &graph), Err(CodecError::InvalidIdSequence));
        let (mut doc, graph) = document();
        doc["next_id"] = json!(99);
        assert_eq!(read(&doc, &graph), Err(CodecError::InvalidIdSequence));
    }

    #[test]
    fn zero_dimensions_lengths_and_invalid_digest_are_rejected() {
        for field in ["byte_len", "width", "height"] {
            let (mut doc, graph) = document();
            doc["assets"][0][field] = json!(0);
            assert_eq!(read(&doc, &graph), Err(CodecError::Syntax));
        }
        for bad in [json!([0]), json!(vec![256; 32]), json!("sha256:bad")] {
            let (mut doc, graph) = document();
            doc["assets"][0]["digest"] = bad;
            assert_eq!(read(&doc, &graph), Err(CodecError::Syntax));
        }
    }

    #[test]
    fn unknown_image_source_or_status_is_rejected() {
        for (path, value) in [
            ("/assets/0/image_kind", "svg"),
            ("/assets/0/source_kind", "verified"),
            ("/reports/0/visuals/status", "legacy_unspecified"),
        ] {
            let (mut doc, graph) = document();
            *doc.pointer_mut(path).unwrap() = json!(value);
            assert_eq!(read(&doc, &graph), Err(CodecError::Syntax));
        }
    }

    #[test]
    fn malformed_typed_references_are_rejected() {
        for origin in ["C1/S3", "step:C01/S3", "cycle:C1", "step:C1/S0"] {
            let (mut doc, graph) = document();
            doc["assets"][0]["origin"] = json!(origin);
            assert_eq!(read(&doc, &graph), Err(CodecError::InvalidReference));
        }
    }

    #[test]
    fn duplicate_report_is_not_silently_overwritten() {
        let (mut doc, graph) = document();
        let duplicate = doc["reports"][0].clone();
        doc["reports"].as_array_mut().unwrap().push(duplicate);
        assert_eq!(read(&doc, &graph), Err(CodecError::DuplicateReport));
    }

    #[test]
    fn missing_closed_graph_report_and_kind_mismatch_are_rejected() {
        let (doc, mut graph) = document();
        graph.reports.remove(&step(1, 3));
        assert_eq!(read(&doc, &graph), Err(CodecError::MissingClosedReport));
        graph
            .reports
            .insert(step(1, 3), ClosedReportKind::Step(NodeKind::Define));
        assert_eq!(read(&doc, &graph), Err(CodecError::ReportKindMismatch));
    }

    #[test]
    fn dangling_reference_is_rejected() {
        let (mut doc, graph) = document();
        doc["reports"][0]["visuals"]["uses"][0]["asset"] = json!(99);
        assert!(matches!(
            read(&doc, &graph),
            Err(CodecError::InvalidReport(
                VisualValidationError::UnavailableAsset { .. }
            ))
        ));
    }

    #[test]
    fn origin_must_be_verify_and_link_back_to_asset() {
        let (mut doc, graph) = document();
        doc["assets"][0]["origin"] = json!("step:C2/S1");
        assert_eq!(read(&doc, &graph), Err(CodecError::InvalidOrigin));
        let (mut doc, mut graph) = document();
        doc["assets"][0]["origin"] = json!("step:C3/S3");
        graph
            .reports
            .insert(step(3, 3), ClosedReportKind::Step(NodeKind::Verify));
        assert_eq!(read(&doc, &graph), Err(CodecError::InvalidOrigin));
    }

    #[test]
    fn references_to_future_assets_are_rejected_by_graph_witness() {
        let (doc, mut graph) = document();
        graph.available = false;
        assert_eq!(read(&doc, &graph), Err(CodecError::UnavailableAtClose));
    }

    #[test]
    fn blank_source_and_duplicate_uses_are_rejected() {
        let (mut doc, graph) = document();
        doc["assets"][0]["source_description"] = json!("\n ");
        assert_eq!(read(&doc, &graph), Err(CodecError::InvalidMetadata));
        let (mut doc, graph) = document();
        let duplicate = doc["reports"][0]["visuals"]["uses"][0].clone();
        doc["reports"][0]["visuals"]["uses"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(matches!(
            read(&doc, &graph),
            Err(CodecError::InvalidReport(
                VisualValidationError::DuplicateAsset { .. }
            ))
        ));
    }

    #[test]
    fn blank_none_reason_and_missing_follow_up_are_rejected() {
        for (visuals, expected) in [
            (
                json!({"status":"not_needed","reason":"\n "}),
                VisualValidationError::BlankReason,
            ),
            (
                json!({"status":"missing","reason":"누락","follow_up":" ","uses":[]}),
                VisualValidationError::BlankFollowUp,
            ),
        ] {
            let (mut doc, graph) = document();
            doc["reports"][0]["visuals"] = visuals;
            assert_eq!(read(&doc, &graph), Err(CodecError::InvalidReport(expected)));
        }
    }

    #[test]
    fn output_limit_stops_before_appending_excess_bytes() {
        use std::io::Write;
        let mut output = LimitedOutput {
            bytes: vec![b' '; MAX_DOCUMENT_BYTES - 1],
            exceeded: false,
        };
        assert_eq!(output.write(b"a").unwrap(), 1);
        assert_eq!(output.bytes.len(), MAX_DOCUMENT_BYTES);
        assert!(output.write(b"b").is_err());
        assert!(output.exceeded);
        assert_eq!(output.bytes.len(), MAX_DOCUMENT_BYTES);
    }

    #[test]
    fn encode_rejects_oversized_metadata_and_invalid_id_state() {
        let (mut registry, graph) = fixture();
        registry.next_id = 99;
        assert_eq!(
            encode(&registry, &graph),
            Err(CodecError::InvalidIdSequence)
        );
        registry.next_id = 2;
        registry
            .assets
            .values_mut()
            .next()
            .unwrap()
            .metadata
            .source_description = "x".repeat(MAX_DOCUMENT_BYTES);
        assert_eq!(encode(&registry, &graph), Err(CodecError::TooLarge));
    }

    #[test]
    fn input_exactly_at_limit_is_not_rejected_for_size() {
        let (_, graph) = fixture();
        let mut bytes = br#"{"version":99}"#.to_vec();
        bytes.resize(MAX_DOCUMENT_BYTES, b' ');
        assert_eq!(decode(&bytes, &graph), Err(CodecError::Version(99)));
    }

    #[test]
    fn removing_new_verify_declaration_is_not_confused_with_legacy_omission() {
        let (mut doc, graph) = document();
        doc["reports"].as_array_mut().unwrap().remove(0);
        assert_eq!(
            read(&doc, &graph),
            Err(CodecError::MissingDeclarationRecord)
        );
        let empty = VisualRegistry::default();
        assert_eq!(
            encode(&empty, &graph),
            Err(CodecError::MissingDeclarationRecord)
        );
    }
}
