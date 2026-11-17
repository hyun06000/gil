// Shared vocabulary for the chain-interview evaluation package. Kept in one place so the
// fixture checker, the result checker and the rubric file cannot drift apart silently.

export const CASE_SCHEMA = 'gil.chain-interview.case/1';
export const RESULT_SCHEMA = 'gil.chain-interview.result/1';
export const COMPARISON_SCHEMA = 'gil.chain-interview.comparison/1';
export const RUBRIC_SCHEMA = 'gil.chain-interview.rubric/1';

// Judgement statuses. Same words as tools/scenarios: a status that was not produced by an
// observation is never promoted to pass.
export const STATUSES = ['pass', 'fail', 'unknown', 'not_run'];
export const VERDICT_STATUSES = ['pass', 'fail'];

export const CASE_CATEGORIES = [
  'ambiguous_request',            // 1. 모호한 요청
  'multi_goal',                   // 2. 복수 목표
  'conflicting_constraints',      // 3. 상충 제약·불명확한 우선순위
  'missing_inputs',               // 4. 입력·선행 결과·권한 부족
  'already_specific',             // 5. 추가 질문 불필요
  'proposal_omits_constraints',   // 6. 제약·비범위·불확실성 누락 제안
  'over_fragmented_items',        // 7. 독립 결과 가치 없는 실행 항목
  'partial_define_claimed_complete', // 8. 일부만 Define하고 전체 완료로 간주
];

export const STAGES = ['interview', 'proposal', 'define'];
export const EXAMPLE_KINDS = ['question', 'proposal', 'define', 'chat_message'];
export const NECESSITY = ['required', 'optional', 'must_not_ask'];
export const CHOICE_ROLES = ['option', 'unsure', 'free_text'];
export const APPROVAL_KINDS = ['none', 'synthetic', 'human_click', 'unknown'];
export const CASE_RESULT_STATUSES = ['not_run', 'evaluated', 'unknown'];
export const JUDGE_KINDS = ['human', 'model'];

// Structural checks that both checkers can recompute from content alone. Their ids are the
// `automatic` vocabulary of the rubric.
export const STRUCTURAL_CHECKS = [
  'proposal_shape',
  'proposal_checklist_matches_items',
  'basis_refs_canonical',
  'question_escape_paths',
  'define_links_item',
  'completion_claims_backed',
];

// Fixture-level checks (case set integrity).
export const FIXTURE_CHECKS = [
  'case_schema',
  'case_id_matches_filename',
  'case_ids_unique',
  'required_inputs_present',
  'reference_integrity',
  'examples_present',
  'bad_example_violations_declared',
  'good_example_has_no_violation',
  'comparison_shape',
];

// Result-level checks (result file consistency).
export const RESULT_CHECKS = [
  'result_schema',
  'run_metadata',
  'case_coverage',
  'not_run_preserved',
  'unknown_preserved',
  'verdict_has_basis',
  'semantic_judge_is_person_or_model',
  'synthetic_approval_not_human',
  'structural_recomputed',
  'criteria_coverage',
  'prepared_evidence_integrity',
];

// Judgements only a human record can settle. A synthetic App decision never earns pass here.
export const HUMAN_ONLY_CRITERIA = ['human_approval_real'];

export const FAILURE_MODES = [
  'unnecessary_question', 'missing_required_question', 'bundled_questions', 'leading_question',
  'intent_expansion', 'answer_dropped', 'vague_statement', 'constraint_dropped',
  'out_of_scope_dropped', 'uncertainty_hidden', 'over_fragmented', 'bundled_items',
  'done_when_unobservable', 'define_wider_than_item', 'define_narrower_than_item',
  'completion_overclaim', 'conflict_resolved_silently', 'assumed_missing_input',
  'credential_requested', 'synthetic_approval_as_human', 'basis_refs_invalid', 'checklist_mismatch',
  'work_inside_interview',
];

export const STEP_REF = /^step:C\d+\/S\d+$/;
