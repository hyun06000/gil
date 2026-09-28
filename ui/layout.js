// **Step DAG 의 자리를 정한다** — 좌표까지만, 그리기는 하지 않는다.
//
// GIL Host UI Model §6.1 이 정한 기준 표현을 그대로 옮긴 것이다. 사실은 한 글자도 만들지
// 않는다 — 부모·형제·revisit 은 `MonitorViewV1` 이 말한 참조에서만 읽고, 글자나 좌표에서
// 짐작하지 않는다.
//
// ```text
// MonitorViewV1 + 접힘 상태 → place() → { nodes, edges, cycles, width, height }
// ```
//
// 아래 행/열 설명은 기본 세로형의 논리 좌표다. horizontal 은 같은 사실을 투영해
// 시간은 오른쪽, 형제 lane 은 아래쪽으로 표시한다. 되돌아감은 왼쪽 과거로 향한다.
//
// # 열
//
//   뿌리                     0 번 열
//   맏이(첫 자식)             부모의 열을 이어 쓴다      ← 순차 진행은 곧게 내려간다
//   그 뒤의 형제              오른쪽의 새 열              ← 생성 순서대로
//
// 형제인지 아닌지는 **같은 `parent_cycle_ref` 를 가진 Cycle 이 시간선에서 앞에 있었는가**로
// 안다. 시간이 지났다는 이유만으로는 열이 바뀌지 않는다.
//
// # 행
//
// Step 하나가 행 하나를 배타적으로 쓴다. 접힌 Cycle 은 그 안의 Step 수와 무관하게 **한 행**만
// 쓰므로, 접을수록 뒤의 것들이 위로 당겨지고 아낀 높이가 쌓인다.
//
// **한 행에는 언제나 Cycle 하나만 있다.** 그래서 가로로 뻗는 선분은 제 Cycle 의 상자만
// 만난다 — 남의 상자나 남의 node 를 가로지를 길이 없다. 아래의 되돌아감 진입 구간이
// 안전한 이유가 이것이다.
//
// # 방향은 절대적이다 (§6.1-3 · §6.1-4 · §12-19)
//
//   형제 생성   **언제나 오른쪽**
//   되돌아감     **언제나 왼쪽** — 전용 gutter 를 탄다
//
// 자리가 없다고 방향을 뒤집지 않는다. 대신 **폭을 넓힌다**. 그래서 `place()` 는 좌표를 주기
// 전에 gutter 가 몇 칸 필요한지부터 세고, 그만큼 `padLeft` 를 오른쪽으로 밀어 둔다. 음수
// 좌표가 나올 여지를 아예 없애는 것이다 — DOM 이나 SVG 가 선을 잘라 감추게 두지 않는다.

export const ROW_H = 32;
export const COL_W = 66;
export const PAD_TOP = 26;
export const NODE_R = 6.5;
export const SELECTED_NODE_R = 9.5;
export const CYCLE_R = 13;
export const BOX_W = 52;
/** Cycle 경계 상자 사이의 틈 — 접기 control 이 앉을 자리이기도 하다. */
export const CYCLE_GAP = 16;
/** 가장 왼쪽 Cycle 상자와 revisit gutter 사이의 최소 여백.
 *
 * 가장 왼쪽에 있는 것은 **상자의 왼쪽 가장자리**다. node 의 focus ring 은 반지름 + 5px
 * (`outline: 2px` + `outline-offset: 3px`)이라 접힌 Cycle 원에서도 18px, 상자 가장자리인
 * 26px 안쪽이다. Cycle 이름도 상자 안에 있다. 그러니 상자에서 물러나면 전부 비킨다. */
export const GUTTER_GAP = 12;
/** 뜻이 다른 되돌아감이 겹칠 때 한 칸씩 **더 왼쪽으로**. 오른쪽으로 보내지 않는다. */
export const GUTTER_STEP = 10;
/** Horizontal lane centre → closest upper return gutter. The Cycle top is 16px
 * above its centre: 28px leaves 12px clear, and a folded circle (r13) still has
 * 7px of straight approach before the 8px arrowhead. */
export const HORIZONTAL_RETURN_CLEARANCE = 28;
/** 가장 왼쪽 gutter 바깥에 선 굵기와 여유로 남기는 canvas 여백. */
export const CANVAS_EDGE = 6;
/** 오른쪽 끝 여백 — 형제 분기가 쓸 수 있는 폭은 layout 이 먼저 확보한다(§6.1-3). */
export const PAD_RIGHT = 30;

/** 그 자리의 반지름 — 접힌 Cycle 은 더 큰 원이다. */
export const radiusOf = (node) => (node.kind === "cycle" ? CYCLE_R : NODE_R);

/** 이 Cycle 이 제 부모의 맏이인가 — 그러면 부모의 열을 이어 쓴다. */
function columns(timeline) {
  const column = new Map();
  const bornFirst = new Map(); // parent_cycle_ref → 그 부모의 맏이
  let nextFree = 0;
  for (const cycle of timeline) {
    const parent = cycle.parent_cycle_ref;
    if (parent === null || parent === undefined) {
      column.set(cycle.cycle_ref, nextFree++);
      continue;
    }
    if (!bornFirst.has(parent)) {
      bornFirst.set(parent, cycle.cycle_ref);
      // 맏이는 부모의 열을 잇는다. 부모가 그림에 없으면 새 열을 연다.
      const inherited = column.has(parent) ? column.get(parent) : nextFree++;
      column.set(cycle.cycle_ref, inherited);
      continue;
    }
    // **진짜 형제다.** 생성 순서대로 오른쪽의 다음 열을 쓴다.
    column.set(cycle.cycle_ref, nextFree++);
  }
  return column;
}

/**
 * 자리를 정한다.
 *
 * @param {object} view canonical MonitorViewV1
 * @param {Set<string>} collapsed 접어 둔 Cycle 의 주소
 */
export function place(view, collapsed, orientation = "vertical") {
  if (!["vertical", "horizontal"].includes(orientation)) throw new Error("unknown DAG orientation");
  const horizontal = orientation === "horizontal";
  const timeGap = horizontal ? 40 : ROW_H;
  const cycleGap = horizontal ? 24 : CYCLE_GAP;
  const column = columns(view.timeline);
  const nodes = [];          // { kind:"step"|"cycle", ref, cycleRef, row, col, x, y, ... }
  const cycles = [];         // { ref, col, fromRow, toRow, collapsed, ... }
  const byRef = new Map();
  let row = 0;
  // Cycle 이 바뀔 때마다 조금씩 벌린다. 행 번호는 그대로라 **Step 의 시간 순서는 흔들리지
  // 않고**, 경계 상자와 접기 control 만 서로 닿지 않게 된다.
  let gap = 0;

  for (const cycle of view.timeline) {
    const col = column.get(cycle.cycle_ref);
    const isCollapsed = collapsed.has(cycle.cycle_ref);
    const fromRow = row;
    if (row > 0) gap += cycleGap;

    if (isCollapsed || cycle.steps.length === 0) {
      // **한 행만 쓴다.** 안의 Step 이 몇이든.
      const node = {
        kind: "cycle",
        ref: cycle.cycle_ref,
        cycleRef: cycle.cycle_ref,
        cycleKind: cycle.kind,
        stepCount: cycle.steps.length,
        row,
        col,
        x: 0, // 뒤에서 준다 — gutter 가 몇 칸인지 알아야 원점이 정해진다
        y: PAD_TOP + row * timeGap + gap,
      };
      nodes.push(node);
      byRef.set(node.ref, node);
      row += 1;
      cycles.push({
        ref: cycle.cycle_ref,
        kind: cycle.kind,
        col,
        fromRow,
        toRow: fromRow,
        collapsed: true,
        relation: cycle.relation_to_current,
      });
      continue;
    }

    for (const step of cycle.steps) {
      const node = {
        kind: "step",
        ref: step.step_ref,
        cycleRef: cycle.cycle_ref,
        stepKind: step.kind,
        state: step.state,
        summary: step.summary,
        here: step.step_ref === view.current.step_ref,
        row,
        col,
        x: 0, // 뒤에서 준다
        y: PAD_TOP + row * timeGap + gap,
      };
      nodes.push(node);
      byRef.set(node.ref, node);
      row += 1;
    }
    cycles.push({
      ref: cycle.cycle_ref,
      kind: cycle.kind,
      col,
      fromRow,
      toRow: row - 1,
      collapsed: false,
      relation: cycle.relation_to_current,
    });
  }

  // ── 줄 ────────────────────────────────────────────────────────────
  //
  // 생성 edge 는 **그 Cycle 의 첫 node** 로 들어간다. 같은 열이면 짧은 수직선이고, 열이
  // 다르면 출발점에서 오른쪽으로 짧게 갈라진 뒤 새 열을 따라 내려간다. 크게 휘지 않는다.
  const edges = [];
  const firstOf = (cycleRef) => nodes.find((node) => node.cycleRef === cycleRef);
  const lastOf = (cycleRef) => [...nodes].reverse().find((node) => node.cycleRef === cycleRef);

  for (const cycle of view.timeline) {
    const head = firstOf(cycle.cycle_ref);
    if (!head) continue;

    // 같은 Cycle 안의 진행 — 행과 행 사이의 짧은 수직선.
    const mine = nodes.filter((node) => node.cycleRef === cycle.cycle_ref);
    for (let at = 1; at < mine.length; at += 1) {
      edges.push({ kind: "grow", from: mine[at - 1], to: mine[at] });
    }

    if (cycle.parent_cycle_ref) {
      const tail = lastOf(cycle.parent_cycle_ref);
      if (tail && tail.row < head.row) {
        edges.push({ kind: "grow", from: tail, to: head });
      }
    }
    // **되돌아감의 점선은 새 Cycle 에서 나가지 않는다**(§6.1-4 · §12-18).
    //
    // 완료된 전환에서 두 참조는 서로 다른 것을 가리킨다(Cycle Model 「전이가 Entry
    // Snapshot 을 정한다」).
    //
    //   revisit_from   이 갈래를 **낳은 실패 Cycle**
    //   parent         **실제로 되돌아간 목표 Cycle** — 새 Cycle 의 구조적 부모이자
    //                  Entry Snapshot 의 출처
    //
    // 그러므로 일어난 일은 "새 Cycle 이 실패 Cycle 로 갔다" 가 아니라 "실패 Cycle 에서
    // 목표 Cycle 로 되돌아갔고, 그 아래에 새 Cycle 이 열렸다" 이다. 점선은 그 되돌아감
    // 자체를 그리고, 새 Cycle 로 들어오는 것은 위의 생성 실선이다.
    //
    // Cycle Entry·Exit 은 Step 이 아니므로(§7) 각 Cycle 의 **마지막 표시 node** 를 Exit
    // 경계의 시각적 anchor 로 쓴다. 접혀 있으면 그 Cycle 의 원이 그 자리를 대신한다.
    // 가상 Step 을 만들지 않는다.
    if (cycle.revisit_from_cycle_ref && cycle.parent_cycle_ref) {
      const failed = lastOf(cycle.revisit_from_cycle_ref);
      const target = lastOf(cycle.parent_cycle_ref);
      if (failed && target) edges.push({ kind: "revisit", from: failed, to: target });
    }
  }

  // ── gutter ────────────────────────────────────────────────────────
  //
  // 되돌아감은 전부 **왼쪽** 으로 간다. 몇 줄이 동시에 왼쪽을 쓰는지 먼저 세고, 그만큼
  // 원점을 오른쪽으로 밀어 자리를 만든다. **방향을 바꾸는 대신 폭을 넓힌다**(§6.1-4).
  //
  // 쓸 수 있는 칸수는 되돌아감의 **개수**로 잡는다 — 접고 펴도 변하지 않는 값이라, 접었다
  // 폈다 할 때 Graph 가 좌우로 흔들리지 않는다. 실제로 쓰는 칸은 겹칠 때만 늘어난다.
  const back = edges.filter((edge) => edge.kind === "revisit");
  const lanes = Math.max(0, back.length - 1);
  const padLeft = BOX_W / 2 + GUTTER_GAP + lanes * GUTTER_STEP + CANVAS_EDGE;

  for (const node of nodes) node.x = padLeft + node.col * COL_W;

  // 뜻이 다른 줄이 **완전히 겹쳐 구분되지 않으면** 한 칸 더 왼쪽으로 물린다.
  // 되돌아감은 언제나 위로 향하므로 세로 구간은 [목표, 출발] 이다.
  const taken = []; // lane 번호 → 이미 그 lane 이 쓰고 있는 세로 구간들
  for (const edge of back) {
    const top = Math.min(edge.from.y, edge.to.y);
    const bottom = Math.max(edge.from.y, edge.to.y);
    let lane = 0;
    while (
      taken[lane] &&
      taken[lane].some((one) => one.top < bottom && top < one.bottom)
    ) {
      lane += 1;
    }
    (taken[lane] ||= []).push({ top, bottom });
    edge.lane = lane;
    // 0번이 상자에 가장 가깝고, 번호가 커질수록 더 왼쪽이다.
    edge.gutter = padLeft - BOX_W / 2 - GUTTER_GAP - lane * GUTTER_STEP;
  }

  const wide = column.size ? Math.max(...column.values()) + 1 : 1;
  const width = padLeft + Math.max(1, wide) * COL_W + PAD_RIGHT;
  const height = PAD_TOP * 2 + Math.max(1, row) * timeGap + gap;
  if (horizontal) {
    // row/col remain logical time/lane, not new domain facts. Only presentation is
    // projected: time goes right, younger siblings take lower lanes. Use the
    // horizontal boundary's height, not the vertical boundary's half-width, to
    // keep returns close. Extra overlapping gutters retain their own spacing.
    const top = CANVAS_EDGE + lanes * GUTTER_STEP + HORIZONTAL_RETURN_CLEARANCE;
    for (const node of nodes) [node.x, node.y] = [node.y, top + node.col * COL_W];
  }
  return { nodes, edges, cycles, byRef, orientation,
    width: horizontal ? height : width,
    height: horizontal ? Math.max(90, ...nodes.map((n) => n.y + 52)) : height,
    padLeft, gutters: taken.length };
}

/** The boundary owns its header/control space; it never changes a node's facts. */
export function cycleBounds(plan, cycle) {
  const mine = plan.nodes.filter((n) => n.cycleRef === cycle.ref);
  const first = mine[0], last = mine.at(-1);
  if (!first) return null;
  return plan.orientation === "horizontal"
    ? { x: first.x - 26, y: first.y - 16, width: last.x - first.x + 52, height: 60 }
    : { x: first.x - BOX_W / 2, y: first.y - 17, width: BOX_W, height: last.y - first.y + 30 };
}

/** Horizontal routes. Grow moves right. A revisit has exactly two bends:
 * node TOP → upper gutter → LEFT into the past → target TOP, pointing DOWN.
 * No side stubs may share a growth edge. Vertical legs use the endpoints' own
 * time slots. Selection changes only the visible radius, never the relationship. */
export function horizontalEdge(edge, selectedStep = null) {
  const { from, to } = edge;
  const radius = node => node.kind === "step" && node.ref === selectedStep ? SELECTED_NODE_R : radiusOf(node);
  if (edge.kind === "grow") {
    const start = [from.x + radius(from), from.y];
    const end = [to.x - radius(to), to.y];
    return { points: from.col === to.col ? [start, end]
      : [start, [from.x + 16, from.y], [from.x + 16, to.y], end] };
  }
  const exit = from.y - radius(from), land = to.y - radius(to);
  return {
    points: [[from.x, exit], [from.x, edge.gutter], [to.x, edge.gutter], [to.x, land - 8]],
    tip: [to.x, land],
  };
}

/** Card indicator geometry, shared by both orientations. The shaft ends at the
 * head's BASE, never its tip. The head follows the shaft even on a diagonal. */
export function cardArrow(from, tip) {
  const dx = tip[0] - from[0], dy = tip[1] - from[1];
  const length = Math.hypot(dx, dy);
  const ux = dx / (length || 1), uy = dy / (length || 1);
  const headLength = Math.min(9, length), halfWidth = 5.5 * headLength / 9;
  const base = [tip[0] - ux * headLength, tip[1] - uy * headLength];
  return {
    shaft: [from, base],
    head: [tip, [base[0] - uy * halfWidth, base[1] + ux * halfWidth],
      [base[0] + uy * halfWidth, base[1] - ux * halfWidth]],
  };
}

/** Find space below the selected horizontal node, without covering any Cycle,
 * node, or control. A long card expands the canvas rather than clipping content. */
export function horizontalCard(plan, anchor, width, height) {
  const x = Math.max(8, anchor.x - width / 2);
  let y = anchor.y + 60;
  const obstacles = [
    ...plan.cycles.filter((c) => !c.collapsed).map((c) => cycleBounds(plan, c)),
    ...plan.nodes.map((n) => ({ x: n.x - radiusOf(n) - 6, y: n.y - radiusOf(n) - 6,
      width: (radiusOf(n) + 6) * 2, height: (radiusOf(n) + 6) * 2 })),
  ];
  for (;;) {
    const hits = obstacles.filter((b) => x < b.x + b.width + 8 && x + width + 8 > b.x &&
      y < b.y + b.height + 8 && y + height + 8 > b.y);
    if (!hits.length) return { x, y };
    y = Math.max(...hits.map((b) => b.y + b.height)) + 16;
  }
}

/**
 * 되돌아감이 지나는 길 — **언제나 왼쪽**이다(§6.1-4 · §12-19).
 *
 * ```text
 *   목표 ●───▶┐   land   목표의 **왼쪽** 가장자리 · 화살촉은 오른쪽
 *             │   gutter  가장 왼쪽 상자보다 더 왼쪽, layout 이 잡아 둔 전용 통로
 *   출발 ●◀───┘   exit   출발의 **왼쪽** 가장자리
 * ```
 *
 * 출발과 목표가 같은 열이어도, 왼쪽이 좁아 보여도 오른쪽으로 물러서지 않는다. 자리는
 * `place()` 가 `padLeft` 로 이미 만들어 두었다.
 *
 * 화살촉이 오른쪽을 향하는 것은 **출발에서 목표로 가는 진행 방향**이기 때문이다. 왼쪽
 * gutter 를 타고 올라와 오른쪽으로 꺾어 목표에 닿는다.
 */
export function revisitRoute(edge) {
  return {
    corridor: edge.gutter,
    exit: edge.from.x - radiusOf(edge.from),
    land: edge.to.x - radiusOf(edge.to),
    approach: 1,
    lane: edge.lane,
  };
}

/** 접힌 Cycle 안의 Step 은 그 Cycle 의 원으로 투영된다(§6.1-9). */
export function projectSelection(plan, stepRef, cycleRefOfStep) {
  if (!stepRef) return null;
  if (plan.byRef.has(stepRef)) return plan.byRef.get(stepRef);
  if (cycleRefOfStep && plan.byRef.has(cycleRefOfStep)) return plan.byRef.get(cycleRefOfStep);
  return null;
}
