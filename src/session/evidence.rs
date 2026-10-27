//! New writes must leave inspectable evidence in the observed world. Old records remain readable.
//! This is the filesystem transaction boundary shared by CLI, ordinary MCP and typed visual MCP.
use super::*;
use std::collections::BTreeSet;

fn refused(reason: impl Into<String>) -> SessionError {
    SessionError::Evidence(reason.into())
}

impl ProjectSession {
    pub(super) fn require_verify_evidence(
        &self, report: &Report, world: &artifact::ManifestAddress,
    ) -> Result<(), SessionError> {
        for field in ["execution", "result"] {
            if report.get(field).is_none_or(|s| s.trim().is_empty()) {
                return Err(refused(format!("{field}에 실제 실행과 관측 결과를 적는다")));
            }
        }
        let declared = report.get("evidence_files").unwrap_or("");
        let mut paths = BTreeSet::new();
        for line in declared.lines().filter(|s| !s.trim().is_empty()) {
            let path = crate::visual_report::file::relative(line)
                .map_err(|_| refused("evidence_files는 프로젝트 안의 안전한 상대 파일 경로를 한 줄에 하나씩 적는다"))?;
            if !paths.insert(path) || paths.len() > 32 {
                return Err(refused("evidence_files는 중복 없이 1~32개 파일을 적는다"));
            }
        }
        if paths.is_empty() {
            return Err(refused("채팅과 Report 본문만으로 Verify를 닫을 수 없다. 관측 문서·실행 결과·시각자료를 실제 파일로 저장하고 evidence_files에 상대 경로를 적는다"));
        }
        let objects = self.store()?;
        let found = objects.read_manifest(world)
            .map_err(|e| SessionError::Object { said: e.to_string() })?;
        let baseline = self.project.world_snapshot();
        let address = self.project.world_manifest(baseline)
            .ok_or(SessionError::UnknownWorld { world: baseline })?;
        let before = objects.read_manifest(address)
            .map_err(|e| SessionError::Manifest { world: baseline, said: e.to_string() })?;
        let empty = artifact::digest_of(std::io::empty()).expect("empty reader");
        let mut fresh = false;
        for path in paths {
            let digest = found.get(&path)
                .ok_or_else(|| refused(format!("증거 파일이 확정할 Snapshot에 없다: {path}")))?;
            if digest == &empty { return Err(refused(format!("빈 파일은 증거가 아니다: {path}"))); }
            fresh |= before.get(&path) != Some(digest);
        }
        if !fresh {
            return Err(refused("지정한 증거가 모두 이전 Snapshot과 같다. 읽기·검사만 했어도 이번 관측 결과를 새 문서나 갱신한 실행 기록으로 남긴다"));
        }
        Ok(())
    }

    /// These references are structural facts, not invented learning or retrospective reports.
    pub(super) fn link_learning(&self, mut report: Report) -> Result<Report, SessionError> {
        let cycle = self.project.current_cycle();
        if cycle.kind() != CycleKind::Experiment { return Ok(report); }
        let Some(here) = cycle.step_now_open() else { return Ok(report); };
        let node = cycle.steps().node(here).expect("open step");
        let (text, field, source_kind) = match node.kind {
            NodeKind::Analysis => ("interpretation", "evidence.verify_ref", NodeKind::Verify),
            NodeKind::Outcome => ("lesson", "learning.analysis_ref", NodeKind::Analysis),
            _ => return Ok(report),
        };
        if report.get(text).is_none_or(|s| s.trim().is_empty()) {
            return Err(refused(format!("{text}에 관측에서 얻은 해석·배움을 적는다. 빈 내용으로 자아의 성장을 기록하지 않는다")));
        }
        let lineage = cycle.steps().lineage(here).map_err(|e| refused(e.to_string()))?;
        let source = lineage.iter().rev().find(|n| n.kind == source_kind && n.is_closed())
            .ok_or_else(|| refused("현재 계보에 닫힌 근거 단계가 없다"))?;
        let expected = cycle.step_ref(source.id).to_string();
        if report.get(field).is_some_and(|given| given != expected) {
            return Err(refused(format!("{field}는 현재 계보의 근거 {expected}를 가리켜야 한다")));
        }
        report.insert(field, expected);
        Ok(report)
    }
}
