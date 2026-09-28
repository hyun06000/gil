import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { place, cycleBounds, horizontalEdge, horizontalCard, radiusOf, projectSelection,
  cardArrow, HORIZONTAL_RETURN_CLEARANCE, GUTTER_STEP, COL_W } from './layout.js';

const fixtures = await Promise.all(['reading-one', 'dense', 'first-interview'].map(async name =>
  JSON.parse(await readFile(new URL(`./fixtures/${name}/view.json`, import.meta.url)))));
const overlap = (a,b) => a.x < b.x+b.width && a.x+a.width > b.x && a.y < b.y+b.height && a.y+a.height > b.y;
const facts = p => ({nodes:p.nodes.map(({x,y,...n})=>n), edges:p.edges.map(e=>[e.kind,e.from.ref,e.to.ref]), cycles:p.cycles});

test('card indicator shaft meets the head base, with the tip furthest toward the card', () => {
  for(const [from,tip] of [[[20,20],[20,70]],[[20,20],[80,20]],[[20,20],[75,48]],[[20,70],[45,10]]]) {
    const {shaft,head}=cardArrow(from,tip);
    assert.deepEqual(shaft[0],from);
    assert.deepEqual(head[0],tip);
    const base=head[1].map((value,i)=>(value+head[2][i])/2);
    base.forEach((value,i)=>assert.ok(Math.abs(value-shaft[1][i])<1e-9));
    assert.ok(Math.abs(Math.hypot(tip[0]-base[0],tip[1]-base[1])-9)<1e-9);
    assert.ok(Math.abs(Math.hypot(head[1][0]-head[2][0],head[1][1]-head[2][1])-11)<1e-9);
    const direction=tip.map((value,i)=>value-from[i]);
    const dot=point=>point.reduce((sum,value,i)=>sum+(value-from[i])*direction[i],0);
    assert.ok(dot(tip)>dot(shaft[1]),'body stops behind the head, not at its tip');
    assert.ok(Math.abs((base[0]-from[0])*direction[1]-(base[1]-from[1])*direction[0])<1e-9);
  }
});
test('short card indicators stay finite and never draw a reversed shaft', () => {
  for(const tip of [[0,0],[0,4],[3,4]]) {
    const route=cardArrow([0,0],tip);
    assert.ok([...route.shaft,...route.head].flat().every(Number.isFinite));
    assert.deepEqual(route.shaft[1],[0,0]);
  }
});

test('horizontal is a pure projection; default vertical is unchanged', () => {
  for (const v of fixtures) {
    const before = JSON.stringify(v);
    const vertical = place(v,new Set()), horizontal = place(v,new Set(),'horizontal');
    assert.deepEqual(facts(horizontal),facts(vertical));
    assert.deepEqual(vertical,place(v,new Set(),'vertical'));
    assert.equal(JSON.stringify(v),before);
    assert.throws(()=>place(v,new Set(),'guess'));
  }
});
test('time has exclusive increasing x; sequential nodes stay in one lane; siblings go down', () => {
  for (const v of fixtures) {
    const p = place(v,new Set(),'horizontal');
    p.nodes.slice(1).forEach((n,i)=>assert.ok(n.x>p.nodes[i].x));
    for (const e of p.edges.filter(e=>e.kind==='grow')) {
      assert.ok(e.to.x>e.from.x);
      if(e.from.col===e.to.col) assert.equal(e.from.y,e.to.y);
      else assert.ok(e.to.y>e.from.y);
      const r=horizontalEdge(e);
      assert.ok(r.points.at(-1)[0]>r.points[0][0]);
    }
  }
});
test('every fold combination keeps top-to-top returns with exactly two bends', () => {
  for(const v of fixtures) {
    const open=place(v,new Set(),'horizontal');
    for(let mask=0;mask<2**v.timeline.length;mask++) {
      const folds=new Set(v.timeline.filter((_,i)=>mask&(1<<i)).map(c=>c.cycle_ref));
      const p=place(v,folds,'horizontal');
      assert.ok(p.width<=open.width);
      for(const c of v.timeline) if(c.revisit_from_cycle_ref) {
        const e=p.edges.find(e=>e.kind==='revisit'&&e.from.cycleRef===c.revisit_from_cycle_ref&&e.to.cycleRef===c.parent_cycle_ref);
        assert.ok(e);
        assert.equal(e.from,p.nodes.filter(n=>n.cycleRef===c.revisit_from_cycle_ref).at(-1));
        assert.equal(e.to,p.nodes.filter(n=>n.cycleRef===c.parent_cycle_ref).at(-1));
        const r=horizontalEdge(e);
        assert.equal(r.points.length,4,'three segments, two bends — no side stubs');
        assert.deepEqual(r.points[0],[e.from.x,e.from.y-radiusOf(e.from)]);
        assert.deepEqual(r.points[1],[e.from.x,e.gutter]);
        assert.deepEqual(r.points[2],[e.to.x,e.gutter]);
        assert.deepEqual(r.points[3],[e.to.x,e.to.y-radiusOf(e.to)-8]);
        assert.deepEqual(r.tip,[e.to.x,e.to.y-radiusOf(e.to)]);
        assert.ok(r.points[1][1]<r.points[0][1],'leave upward');
        assert.ok(r.points[2][0]<r.points[1][0],'return leftward into the past');
        assert.ok(r.tip[1]>r.points[3][1],'arrow points down onto the node top');
        for(const b of p.cycles.filter(c=>!c.collapsed).map(c=>cycleBounds(p,c))) {
          assert.ok(e.gutter<b.y);
          assert.ok(b.x>=0&&b.y>=0);
        }
        for(const point of [...r.points,r.tip]) assert.ok(point[0]>=0&&point[1]>=0&&point[0]<=p.width&&point[1]<=p.height);
      }
      for(const c of v.timeline.filter(c=>folds.has(c.cycle_ref)&&c.steps.length))
        assert.equal(projectSelection(p,c.steps[0].step_ref,c.cycle_ref).ref,c.cycle_ref);
    }
  }
});
test('return gutters stay compact without crowding folded nodes or arrowheads', () => {
  assert.equal(HORIZONTAL_RETURN_CLEARANCE,28);
  for(const v of fixtures) for(let mask=0;mask<2**v.timeline.length;mask++) {
    const folds=new Set(v.timeline.filter((_,i)=>mask&(1<<i)).map(c=>c.cycle_ref));
    const p=place(v,folds,'horizontal');
    for(const e of p.edges.filter(e=>e.kind==='revisit')) {
      for(const node of [e.from,e.to])
        assert.equal(node.y-e.gutter,28+node.col*COL_W+e.lane*GUTTER_STEP);
      for(const c of p.cycles.filter(c=>!c.collapsed))
        assert.ok(cycleBounds(p,c).y-e.gutter>=12,'gutter clears every Cycle boundary');
      const r=horizontalEdge(e);
      assert.ok(r.points[3][1]-e.gutter>=7,'folded target retains straight approach before arrowhead');
    }
  }
});
test('top-to-top returns never run along a growth edge, including folded layouts', () => {
  const segments = route => route.points.slice(1).map((point,i)=>[route.points[i],point]);
  const sharesLine = ([a,b],[c,d]) => {
    const vertical = a[0]===b[0] && c[0]===d[0] && a[0]===c[0];
    const horizontal = a[1]===b[1] && c[1]===d[1] && a[1]===c[1];
    if(!vertical && !horizontal) return false;
    const k=vertical?1:0;
    return Math.max(Math.min(a[k],b[k]),Math.min(c[k],d[k])) < Math.min(Math.max(a[k],b[k]),Math.max(c[k],d[k]));
  };
  for(const v of fixtures) for(let mask=0;mask<2**v.timeline.length;mask++) {
    const folds=new Set(v.timeline.filter((_,i)=>mask&(1<<i)).map(c=>c.cycle_ref));
    const p=place(v,folds,'horizontal');
    const growing=p.edges.filter(e=>e.kind==='grow').flatMap(e=>segments(horizontalEdge(e)));
    for(const edge of p.edges.filter(e=>e.kind==='revisit')) {
      const route=horizontalEdge(edge);
      for(const segment of segments({...route,points:[...route.points,route.tip]}))
        assert.ok(growing.every(other=>!sharesLine(segment,other)),`${edge.from.ref}→${edge.to.ref} shares a growth edge`);
      for(const selected of [edge.from.ref,edge.to.ref]) {
        const chosen=horizontalEdge(edge,selected);
        const radius=n=>n.kind==='cycle'?13:n.ref===selected?9.5:6.5;
        assert.equal(chosen.points[0][1],edge.from.y-radius(edge.from));
        assert.deepEqual(chosen.tip,[edge.to.x,edge.to.y-radius(edge.to)]);
      }
    }
  }
});
test('summary cards, including tall reports, cover no nodes or Cycle controls', () => {
  for(const v of fixtures) for(const folds of [new Set(),new Set(v.timeline.map(c=>c.cycle_ref))]) {
    const p=place(v,folds,'horizontal');
    const obstacles=[...p.cycles.filter(c=>!c.collapsed).map(c=>cycleBounds(p,c)),
      ...p.nodes.map(n=>({x:n.x-radiusOf(n)-6,y:n.y-radiusOf(n)-6,width:2*(radiusOf(n)+6),height:2*(radiusOf(n)+6)}))];
    for(const node of p.nodes) for(const height of [70,400]) {
      const card={...horizontalCard(p,node,186,height),width:186,height};
      assert.ok(obstacles.every(b=>!overlap(card,b)));
    }
  }
});
test('500 sequential Steps remain one lane and folding saves all hidden time slots', () => {
  const v={current:{step_ref:null},timeline:Array.from({length:50},(_,i)=>({cycle_ref:`cycle:C${i+1}`,
    parent_cycle_ref:i?`cycle:C${i}`:null,steps:Array.from({length:10},(_,j)=>({step_ref:`step:C${i+1}/S${j+1}`}))}))};
  const open=place(v,new Set(),'horizontal'), closed=place(v,new Set(v.timeline.map(c=>c.cycle_ref)),'horizontal');
  assert.equal(new Set(open.nodes.map(n=>n.y)).size,1);
  assert.equal(closed.nodes.length,50);
  assert.equal(open.width-closed.width,450*40);
});
