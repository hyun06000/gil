// Zoom and minimap arithmetic for the Step DAG viewport. Presentation only: nothing here
// reads a View, changes the plan or leaves the window. The plan's coordinates stay in
// unscaled "stage" pixels; the viewport scales them with one CSS transform and these
// helpers convert between stage pixels, scaled pixels and the minimap's pixels.

export const ZOOM_MIN = 0.4;
export const ZOOM_MAX = 2.5;
export const ZOOM_FACTOR = 1.2;
export const MINIMAP_MAX = { width: 220, height: 130 };
export const MINIMAP_MIN_NODES = 1;

export function clampZoom(zoom) {
  if (!Number.isFinite(zoom)) return 1;
  return Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(zoom * 1000) / 1000));
}

/** One step in or out. `delta > 0` zooms in. */
export function stepZoom(zoom, delta) {
  return clampZoom(delta > 0 ? zoom * ZOOM_FACTOR : zoom / ZOOM_FACTOR);
}

/**
 * Scroll position that keeps the stage point under the pointer still after a zoom change.
 * `pointer` is relative to the viewport's top-left in scaled pixels.
 */
export function zoomAround({ scrollLeft, scrollTop }, pointer, from, to) {
  const stageX = (scrollLeft + pointer.x) / from, stageY = (scrollTop + pointer.y) / from;
  return { scrollLeft: Math.max(0, stageX * to - pointer.x), scrollTop: Math.max(0, stageY * to - pointer.y) };
}

/** The minimap's own size and the stage→minimap scale, fitting the stage into the allowed box. */
export function minimapFrame(stage, box = MINIMAP_MAX) {
  const width = Math.max(1, stage.width), height = Math.max(1, stage.height);
  const scale = Math.min(box.width / width, box.height / height);
  return { scale, width: Math.max(24, Math.round(width * scale)), height: Math.max(16, Math.round(height * scale)) };
}

/** The visible window as a rectangle in minimap pixels, clamped to the minimap. */
export function minimapViewport(frame, stage, zoom, view) {
  const toMini = (v) => (v / zoom) * frame.scale;
  const x = toMini(view.scrollLeft), y = toMini(view.scrollTop);
  const w = toMini(view.clientWidth), h = toMini(view.clientHeight);
  const right = Math.min(frame.width, x + w), bottom = Math.min(frame.height, y + h);
  return { x: Math.min(x, frame.width), y: Math.min(y, frame.height),
    width: Math.max(0, right - Math.min(x, frame.width)), height: Math.max(0, bottom - Math.min(y, frame.height)) };
}

/** Scroll so the stage point behind a minimap pixel sits at the viewport's centre. */
export function scrollToMinimapPoint(frame, zoom, view, point) {
  const stageX = point.x / frame.scale, stageY = point.y / frame.scale;
  return { scrollLeft: Math.max(0, stageX * zoom - view.clientWidth / 2),
    scrollTop: Math.max(0, stageY * zoom - view.clientHeight / 2) };
}

/** The minimap earns its place only when something is out of view at this zoom. */
export function minimapNeeded(stage, zoom, view) {
  return stage.width * zoom > view.clientWidth + 1 || stage.height * zoom > view.clientHeight + 1;
}
