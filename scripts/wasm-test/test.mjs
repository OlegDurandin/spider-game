// In-memory DOM integration, not a browser or visual/layout test.
import assert from 'node:assert/strict';
import {readFile,readdir,writeFile} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import {JSDOM,VirtualConsole} from 'jsdom';
const base='spider.game.v1',slotKey=s=>s==='regular'?base:base+'.'+s;
const fixture=async n=>readFile('/fixtures/'+n+'.json','utf8');
const scenario=process.argv[2];
if(!scenario){
 for(const name of ['navigation','restart','tail','dual','redeal','redeal-restart','victory','won-restart','invitation','legacy-link','invalid']){
  const p=spawnSync(process.execPath,['/qa/test.mjs',name],{encoding:'utf8',timeout:30000});
  process.stdout.write(p.stdout||'');process.stderr.write(p.stderr||'');
  assert.equal(p.status,0,'scenario '+name);
 }
 console.log('WASM/DOM integration passed. No real browser, layout engine or platform dialog was used.');process.exit(0);
}
const virtualConsole=new VirtualConsole();virtualConsole.on('jsdomError',e=>{if(!e.message.includes('Not implemented'))console.error(e.message);});
let query='';if(scenario==='invitation')query='?deal=k1_1_0123456789abcdef';if(scenario==='legacy-link')query='?deal=s1_4_0123456789abcdef';if(scenario==='invalid')query='?deal=k9_1_nope';
const dom=new JSDOM('<!doctype html><html><body><p id="startup">Loading</p></body></html>',{url:'https://qa.invalid/'+query,runScripts:'outside-only',pretendToBeVisual:true,virtualConsole});
const w=dom.window,d=w.document,storage=w.localStorage;
w.TextEncoder=TextEncoder;w.TextDecoder=TextDecoder;w.Request=Request;w.Response=Response;w.Headers=Headers;w.fetch=fetch;
w.matchMedia=()=>({matches:true,addEventListener(){},removeEventListener(){}});
w.SPIDER_CONFIG={publicUrl:'https://game.example/',botUsername:'spidercardgame_bot'};
w.eval(await readFile('/site/sharing.js','utf8'));
const parse=slot=>JSON.parse(storage.getItem(slotKey(slot))||'null');
const active=()=>storage.getItem(base+'.active')||'regular';
const current=()=>parse(active());
const snapshot=()=>Object.fromEntries(Object.keys(storage).filter(k=>k===base||k.startsWith(base+'.klondike_regular')||k.startsWith(base+'.daily_')||k.startsWith(base+'.challenge_')).map(k=>[k,storage.getItem(k)]));
const semantic=g=>JSON.stringify({state:g.state,history:g.history});
const loadStorage=async path=>{for(const [k,v] of Object.entries(JSON.parse(await readFile(path,'utf8'))))storage.setItem(k,v);};
const saveStorage=async path=>writeFile(path,JSON.stringify(Object.fromEntries(Object.keys(storage).map(k=>[k,storage.getItem(k)]))));
if(['restart','invitation','legacy-link','invalid'].includes(scenario))await loadStorage('/tmp/pasyans-navigation.json');
else if(scenario==='redeal-restart')await loadStorage('/tmp/pasyans-redeal.json');
else if(scenario==='won-restart')await loadStorage('/tmp/pasyans-won.json');
else {
 storage.setItem(base,await fixture('spider-legacy4'));
 const name={tail:'tail',dual:'dual',redeal:'redeal-ready',victory:'before-win'}[scenario];
 if(name){storage.setItem(slotKey('klondike_regular'),await fixture('klondike-'+name));storage.setItem(base+'.active','klondike_regular');}
}
const entries=await readdir('/site');const js=entries.find(n=>/^spider-web-.*\.js$/.test(n)),wasm=entries.find(n=>/_bg\.wasm$/.test(n));
let glue=await readFile('/site/'+js,'utf8');
glue=glue.replace(/^export default .*;$/gm,'').replace(/^export \{[^}]+\};$/gm,'').replace(/^export /gm,'').replaceAll('import.meta.url',JSON.stringify('https://qa.invalid/module.js'));
w.eval(glue+'\nwindow.__startWasm=__wbg_init;');const initArgument=new w.Object();initArgument.module_or_path=new w.Uint8Array(await readFile('/site/'+wasm));await w.__startWasm(initArgument);
const settle=async()=>{for(let i=0;i<5;i++)await new Promise(r=>setTimeout(r,20));};await settle();
const button=(text,aria=false)=>[...d.querySelectorAll('button')].find(b=>aria?b.getAttribute('aria-label')===text:b.textContent.replace(/›/g,'').trim()===text);
async function click(text,aria=false){const b=button(text,aria);assert.ok(b,'missing button '+text);assert.ok(!b.disabled,'disabled '+text);assert.ok(!b.closest('[inert]'),'underlay is inert '+text);b.dispatchEvent(new w.MouseEvent('click',{bubbles:true,cancelable:true}));await settle();assert.ok(d.querySelectorAll('[role=dialog]').length<=1,'single panel');}
async function radio(text){const b=[...d.querySelectorAll('[role=radio]')].find(b=>b.textContent.trim()===text);assert.ok(b,'missing radio '+text);b.click();await settle();}
const targets=selector=>[...d.querySelectorAll(selector)].map(el=>Number(el.dataset.column??el.dataset.foundation));
async function parties(text){await click('Пауза',true);await click('Мои партии');const b=[...d.querySelectorAll('.panel-rows button')].find(b=>b.textContent.startsWith(text));assert.ok(b,'party '+text);b.click();await settle();}
try {
 assert.ok(d.querySelector('.game'),'WASM mounted');
 if(scenario==='navigation'){
  const old=JSON.parse(await fixture('spider-legacy4'));assert.equal(semantic(current()),semantic(old));assert.equal(current().kind,'spider');
  await click('Новая игра',true);await radio('Косынка');assert.equal(d.querySelector('.replacement-warning'),null);assert.equal(parse('klondike_regular'),null);
  await click('Закрыть новую игру',true);assert.equal(semantic(current()),semantic(old));
  await click('Новая игра',true);await radio('Косынка');await click('Начать игру');assert.equal(active(),'klondike_regular');assert.equal(current().state.stock.length,24);assert.equal(d.querySelectorAll('.k-column').length,7);assert.equal(d.querySelectorAll('.k-foundation').length,4);
  for(let i=0;i<2;i++){const stock=d.querySelector('.k-stock');stock.click();await settle();}assert.equal(current().state.moves,2);assert.equal(current().history.length,2);
  const k=storage.getItem(slotKey(active()));await click('Пауза',true);await click('Действия с раскладом',true);await click('Как играть');assert.match(d.querySelector('.app-panel').textContent,/52/);await click('Вернуться к столу');await click('Новая игра',true);assert.match(d.querySelector('.replacement-warning').textContent,/Косынку/);await radio('Паук');assert.match(d.querySelector('.replacement-warning').textContent,/Паука/);await click('Закрыть новую игру',true);assert.equal(storage.getItem(slotKey(active())),k);
  await parties('Паук · Обычная партия');assert.equal(semantic(current()),semantic(old));const deal=[...d.querySelectorAll('footer button')].find(b=>b.textContent.includes('Раздать ряд'));deal.click();await settle();assert.equal(current().state.moves,2);assert.equal(current().history.length,2);
  await parties('Косынка · Обычная партия');assert.equal(storage.getItem(slotKey(active())),k);
  await click('Пауза',true);await click('Расклад дня · Паук');await click('Играть');assert.equal(current().kind,'spider');assert.match(active(),/^daily_/);
  await parties('Косынка · Обычная партия');assert.equal(storage.getItem(slotKey(active())),k);await saveStorage('/tmp/pasyans-navigation.json');
 }
 if(scenario==='restart'){assert.equal(active(),'klondike_regular');assert.equal(current().state.moves,2);assert.equal(parse('regular').state.moves,2);assert.ok(Object.keys(storage).some(k=>k.startsWith(base+'.daily_')));await parties('Паук · Обычная партия');assert.equal(current().state.moves,2);}
 if(scenario==='tail'){
  const old=semantic(current());await click('8 червей, столбец 1',true);assert.deepEqual(targets('.k-column.target'),[1]);assert.deepEqual(targets('.k-foundation.target'),[]);assert.match(d.querySelector('.selection-ring').style.top,/34/);
  await click('9 треф, столбец 2',true);assert.deepEqual(current().state.columns[1].map(c=>c.id),[34,20,32]);assert.equal(current().state.moves,1);await click('↶Отменить');assert.equal(semantic(current()),old);
 }
 if(scenario==='dual'){
  await click('Сброс: 5 бубен',true);assert.deepEqual(targets('.k-column.target'),[0]);assert.deepEqual(targets('.k-foundation.target'),[3]);await click('Дом бубен: 4',true);assert.equal(current().state.foundations[3].length,5);await click('Дом бубен: 5',true);await click('6 треф, столбец 1',true);assert.equal(current().state.foundations[3].length,4);assert.deepEqual(current().state.columns[0].map(c=>c.id),[31,43]);
 }
 if(scenario==='redeal'){
  const old=current();assert.equal(old.state.stock.length,0);const first=old.state.waste[0].id;await click('Перевернуть сброс в запас',true);assert.equal(current().state.moves,25);assert.equal(current().state.waste.length,0);assert.equal(current().state.stock.at(-1).id,first);await saveStorage('/tmp/pasyans-redeal.json');
 }
 if(scenario==='redeal-restart'){
  assert.equal(current().state.moves,25);const expected=current().state.stock.at(-1).id;d.querySelector('.k-stock').click();await settle();assert.equal(current().state.waste.at(-1).id,expected);await click('↶Отменить');assert.equal(current().state.moves,25);assert.equal(current().state.waste.length,0);
 }
 if(scenario==='victory'){
  await click('K бубен, столбец 1',true);assert.deepEqual(targets('.k-foundation.target'),[3]);await click('Дом бубен: Q',true);assert.equal(current().state.status,'won');assert.equal(current().state.moves,1);assert.equal(d.querySelectorAll('h2').length,1);await saveStorage('/tmp/pasyans-won.json');
  await click('Вызвать друга');assert.match(d.querySelector('.share-link').value,/deal=k1_1_0123456789abcdef$/);await click('Вернуться к игре');await click('Действия с раскладом',true);await click('Отменить последний ход');assert.equal(current().state.status,'playing');assert.equal(current().state.moves,0);assert.equal(current().state.foundations.flat().length,51);assert.equal(storage.getItem(base+'.best.k1_1_0123456789abcdef'),'1');
 }
 if(scenario==='won-restart'){assert.equal(current().state.status,'won');assert.equal(current().state.foundations.flat().length,52);assert.match(d.querySelector('h2').textContent,/Победа/);}
 if(scenario==='invitation'){
  const before=storage.getItem(base),ordinary=storage.getItem(slotKey('klondike_regular'));await click('Играть');assert.equal(active(),'challenge_k1_1_0123456789abcdef');const golden=JSON.parse(await fixture('klondike-v1'))['k1_1_0123456789abcdef'];assert.deepEqual(current().state,golden);assert.equal(storage.getItem(base),before);assert.equal(storage.getItem(slotKey('klondike_regular')),ordinary);
  await click('Пауза',true);await click('Действия с раскладом',true);await click('Поделиться раскладом');assert.match(d.querySelector('.share-link').value,/deal=k1_1_0123456789abcdef$/);await click('Вернуться к игре');d.querySelector('.k-stock').click();await settle();assert.equal(current().state.waste.at(-1).id,golden.stock.at(-1).id);
  await click('Пауза',true);await click('Действия с раскладом',true);await click('Начать расклад заново');await click('Начать заново');assert.deepEqual(current().state,golden);assert.equal(current().history.length,0);
 }
 if(scenario==='legacy-link'){await click('Играть');assert.equal(current().kind,'spider');assert.equal(current().state.mode,'four');assert.equal(current().deal.seed,81985529216486895);assert.equal(current().state.moves,0);}
 if(scenario==='invalid'){const before=snapshot();assert.match(d.querySelector('.app-panel').textContent,/Неизвестная/);await click('К моей игре');assert.deepEqual(snapshot(),before);assert.equal(active(),'klondike_regular');}
 console.log('PASS WASM/DOM: '+scenario);w.close();process.exit(0);
}catch(e){console.error('FAIL '+scenario,e);w.close();process.exit(1);}
