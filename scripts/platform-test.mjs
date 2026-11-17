import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';
const source = await readFile(new URL('../web/public/platform.js', import.meta.url), 'utf8');
async function run({ search = '', hash = '', sdk, fail = false } = {}) {
  const calls = [], scripts = [], listeners = {}, styles = {}, events = [];
  const window = { dispatchEvent: event => events.push(event.type), addEventListener: (event, fn) => { listeners[event] = fn; }, ...sdk?.(calls) };
  const context = { window, location: { search, hash }, URLSearchParams, Event, console: { info() {} }, document: {
    documentElement: { style: { setProperty: (key, value) => { styles[key] = value; } } },
    createElement: () => ({}), head: { appendChild: script => { scripts.push(script); queueMicrotask(() => fail ? script.onerror() : script.onload()); } },
  } };
  vm.runInNewContext(source, context);
  listeners['spider-ready']();
  await new Promise(resolve => setImmediate(resolve));
  return { calls, scripts, styles, events, window };
}
let r = await run(); assert.equal(r.scripts.length, 0);
r = await run({ hash: '#tgWebAppData=test', sdk: calls => ({ Telegram: { WebApp: {
  ready: () => calls.push('ready'), expand: () => calls.push('expand'), onEvent() {},
  viewportStableHeight: 700, contentSafeAreaInset: { top: 12, bottom: 24 },
} } }) });
assert.deepEqual(r.calls, ['expand', 'ready']); assert.equal(r.styles['--app-height'], '700px'); assert.equal(r.styles['--platform-bottom'], '24px');
r = await run({ search: '?vk_app_id=1', sdk: calls => ({ vkBridge: { send: async method => calls.push(method), subscribe() {} } }) });
assert.deepEqual(r.calls, ['VKWebAppInit']);
await run({ search: '?vk_app_id=1', fail: true });
await run({ hash: '#tgWebAppData=test', fail: true });
console.log('Platform adapters: 5 scenarios passed (browser, Telegram, VK, 2 SDK failures). Mock SDKs only.');
let backClick;
r = await run({ hash: '#tgWebAppData=test', sdk: calls => ({ Telegram: { WebApp: {
  ready() {}, expand() {}, onEvent() {},
  BackButton: { onClick: fn => {backClick=fn;}, show:()=>calls.push('back-show'), hide:()=>calls.push('back-hide') },
} } }) });
assert.equal(typeof backClick,'function');assert.deepEqual(r.calls,['back-hide']);
r.window.spiderSetBackVisible(true);backClick();r.window.spiderSetBackVisible(false);
assert.deepEqual(r.calls,['back-hide','back-show','back-hide']);assert.deepEqual(r.events,['spider-platform-ready','spider-back']);
console.log('Telegram Back: visibility and one app-back event verified.');
