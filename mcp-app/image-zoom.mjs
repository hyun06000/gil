// Reuse the single decoded, verified image. No new URL, request or stored state.
export function imageZoom(img, doc = document) {
  const viewport = doc.createElement('div');
  viewport.className = 'visual-viewport';
  const trigger = doc.createElement('button');
  trigger.type = 'button'; trigger.className = 'visual-image-trigger';
  const hint = doc.createElement('p');
  trigger.append(img); viewport.append(trigger);
  let expanded = false;
  function set(value) {
    expanded = value;
    viewport.classList.toggle('is-original', value);
    const label = value ? '미리보기로 돌아가기' : '원본 크기로 확대';
    trigger.setAttribute('aria-label', label);
    trigger.setAttribute('aria-expanded', String(value));
    hint.textContent = value
      ? `원본 크기 ${img.naturalWidth} × ${img.naturalHeight} · 스크롤로 이동 · Esc로 미리보기`
      : '이미지를 누르면 원본 크기로 볼 수 있습니다.';
    viewport.tabIndex = value ? 0 : -1;
    if (value) viewport.focus({preventScroll:true});
    else { viewport.scrollTop = 0; viewport.scrollLeft = 0; }
  }
  trigger.onclick = () => set(!expanded);
  viewport.onkeydown = event => {
    if (expanded && event.key === 'Escape') {
      event.preventDefault(); set(false); trigger.focus({preventScroll:true});
    }
  };
  set(false);
  return {viewport, hint};
}
