import { t } from '../ui/locale.js';
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
    const label = value ? t('zoom.to_preview') : t('zoom.to_original');
    trigger.setAttribute('aria-label', label);
    trigger.setAttribute('aria-expanded', String(value));
    hint.textContent = value
      ? t('zoom.hint_original', { w: img.naturalWidth, h: img.naturalHeight })
      : t('zoom.hint_preview');
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
