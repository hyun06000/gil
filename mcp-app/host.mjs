// Transport adapter only: no layout, .gil parsing, domain joins, or filesystem paths.
export const RETRY_MS = [2000, 4000, 8000, 16000, 30000, 60000];
export const retryAfter = (failures) => RETRY_MS[Math.min(Math.max(failures - 1, 0), RETRY_MS.length - 1)];
export const RECONCILE_MS = 300_000;

export function connectionMessage(error) {
  switch (error?.code) {
    case 'unknown_scope': case 'reconnect_required':
      return '저장된 프로젝트 연결이 없습니다. 이 프로젝트의 Monitor를 한 번 열어 주세요. 기존 기록은 초기화하지 않습니다.';
    case 'project_missing':
      return '프로젝트 폴더를 찾을 수 없습니다. 폴더나 외장 드라이브가 돌아오면 자동으로 다시 확인합니다. 마지막 그래프는 유지합니다.';
    case 'project_moved':
      return '원래 자리의 프로젝트가 바뀌어 자동 연결을 멈췄습니다. 사용할 폴더를 다시 선택해 주세요. 마지막 그래프는 유지합니다.';
    default:
      return error?.said || '프로젝트 연결을 자동으로 다시 확인 중입니다. 마지막 그래프는 유지합니다.';
  }
}

export function unwrap(result) {
  const data = result?.structuredContent;
  if (result?.isError || !data) {
    throw { code: data?.code || 'unavailable', said: data?.said || 'Monitor 연결을 읽지 못했다' };
  }
  return data;
}

export function makeHost(app, initial, { now = Date.now } = {}) {
  const scope = initial.scope_id;
  const listeners = new Map();
  let seed = initial;
  let cursor = initial.revision;
  let worked = now();
  let nextRead = 0;
  let failures = 0;
  let watchWasLive = initial.watching;
  let disconnected = false;
  const emit = async (event, payload) => { for (const fn of listeners.get(event) || []) await fn(payload); };
  const call = async (name, extra = {}) => {
    try {
      const data = unwrap(await app.callServerTool({ name, arguments: { scope_id: scope, ...extra } }));
      if (data.scope_id !== scope) throw { code: 'unknown_scope', said: '다른 Project 의 응답을 거절했다' };
      if (disconnected) { disconnected = false; await emit('gil://connection', null); }
      return data;
    } catch (err) {
      disconnected = true; await emit('gil://connection', err); throw err;
    }
  };
  const watchNotice = async (live) => {
    if (watchWasLive === live) return;
    watchWasLive = live;
    await emit('gil://say', live ? { code: 'watch_live' }
      : { code: 'watch_unavailable', said: '자동 갱신을 시작하지 못했다 — 수동 새로고침과 5분 재조회를 사용한다' });
  };
  return {
    listProjects: async () => [{ scope_id: scope, label: initial.label }],
    opening: async () => ({ last_selected: scope }),
    async loadView(wanted) {
      if (wanted !== scope) throw { code: 'unknown_scope', said: '이 화면이 연 Project 가 아니다' };
      try {
        const data = seed || await call('gil_monitor_read');
        seed = null;
        if (data.view?.schema_version !== 1) throw { code: 'unsupported_vocabulary', said: 'Monitor 판을 읽지 못했다' };
        cursor = data.revision;
        worked = now(); failures = 0; nextRead = 0;
        await watchNotice(data.watching);
        return data.view;
      } catch (err) {
        failures += 1; nextRead = now() + retryAfter(failures);
        throw err;
      }
    },
    async loadDetail(wanted, stepRef) {
      if (wanted !== scope) throw { code: 'unknown_scope', said: '이 화면이 연 Project 가 아니다' };
      return (await call('gil_monitor_detail', { step_ref: stepRef })).detail;
    },
    listen(event, fn) {
      if (!listeners.has(event)) listeners.set(event, new Set());
      listeners.get(event).add(fn);
      return () => listeners.get(event).delete(fn);
    },
    async tick() {
      const hint = await call('gil_monitor_poll'); // counter only; no world scan
      await watchNotice(hint.watching);
      if (now() >= nextRead && (failures > 0 || hint.revision !== cursor || now() - worked >= RECONCILE_MS)) {
        await emit('gil://refresh');
      }
    },
    refresh: () => emit('gil://refresh'),
  };
}
