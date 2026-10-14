import test from 'node:test';
import assert from 'node:assert/strict';
import {imageZoom} from './image-zoom.mjs';
function element() {
  return {children:[], attrs:{}, classes:new Set(), append(...nodes){this.children.push(...nodes);},
    setAttribute(k,v){this.attrs[k]=v;}, focus(){this.focused=true;},
    get classList(){return {toggle:(k,v)=>v?this.classes.add(k):this.classes.delete(k)};}};
}
test('image click toggles one image without a separate zoom button or source change',()=>{
  const img={...element(),naturalWidth:638,naturalHeight:875,src:'verified-image'};
  const z=imageZoom(img,{createElement:element}), trigger=z.viewport.children[0];
  assert.equal(trigger.children[0],img);
  assert.equal(z.toggle,undefined);
  assert.equal(trigger.attrs['aria-expanded'],'false');
  trigger.onclick();
  assert.equal(trigger.attrs['aria-expanded'],'true');
  assert.ok(z.viewport.classes.has('is-original'));
  assert.match(z.hint.textContent,/638 × 875/);
  assert.equal(z.viewport.tabIndex,0);
  z.viewport.scrollTop=200; z.viewport.scrollLeft=100;
  trigger.onclick();
  assert.equal(z.viewport.scrollTop,0); assert.equal(z.viewport.scrollLeft,0);
  assert.equal(img.src,'verified-image'); assert.equal(trigger.children.length,1);
});
test('Escape collapses and returns keyboard focus; unrelated keys do not collapse',()=>{
  const z=imageZoom({...element(),naturalWidth:1,naturalHeight:1},{createElement:element});
  const trigger=z.viewport.children[0];
  trigger.onclick(); let prevented=false;
  z.viewport.onkeydown({key:'ArrowDown'});
  assert.equal(trigger.attrs['aria-expanded'],'true');
  z.viewport.onkeydown({key:'Escape',preventDefault(){prevented=true;}});
  assert.ok(prevented); assert.ok(trigger.focused);
  assert.equal(trigger.attrs['aria-expanded'],'false');
});
