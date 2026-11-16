import assert from 'node:assert/strict';
import vm from 'node:vm';
import {readFile} from 'node:fs/promises';
const source=await readFile(new URL('../web/public/sharing.js',import.meta.url),'utf8');
const code='s1_2_ffffffffffffffff';
function run(search='',hash='',extra={}){const events=[];const window={SPIDER_CONFIG:{publicUrl:'https://game.example/play?secret=bad#session',botUsername:'spidercardgame_bot',vkAppId:'123'},dispatchEvent:e=>events.push(e.type),...extra};const context={window,location:{search,hash},URL,URLSearchParams,Event:class{constructor(type){this.type=type;}}};vm.runInNewContext(source,context);return {window,context,events};}
let r=run();assert.equal(r.window.spiderDealLink(code),'https://game.example/play?deal='+code);
r=run('?tgWebAppPlatform=ios&auth=secret','#tgWebAppData='+encodeURIComponent('start_param='+code+'&hash=private'),{Telegram:{WebApp:{openTelegramLink:url=>assert.ok(url.startsWith('https://t.me/share/url?'))}}});assert.equal(r.window.spiderIncomingDeal(),code);assert.equal(r.window.spiderDealLink(code),'https://t.me/spidercardgame_bot?startapp='+code);r.window.spiderShareDeal(code,'test');assert.deepEqual(r.events,['spider-share-done']);
r=run('?vk_app_id=123&sign=secret','#/deal/'+code,{vkBridge:{send:(method,args)=>{assert.equal(method,'VKWebAppShare');assert.equal(args.link,'https://vk.com/app123#/deal/'+code);return Promise.resolve({result:false});}}});assert.equal(r.window.spiderIncomingDeal(),code);r.window.spiderShareDeal(code,'test');await new Promise(setImmediate);assert.deepEqual(r.events,['spider-share-done']);
r=run('?deal='+code);assert.equal(r.window.spiderIncomingDeal(),code);r.context.location.hash='#/deal/p1_2';r.context.location.search='';assert.equal(r.window.spiderIncomingDeal(),'p1_2');
r=run('', '#/deal/%zz');assert.equal(r.window.spiderIncomingDeal(),'!invalid-code');
for(const [search,hash] of [['?tgWebAppStartParam='+code,''],['','#tgWebAppStartParam='+code]])assert.equal(run(search,hash).window.spiderIncomingDeal(),code);
r=run('','',{navigator:{share:()=>Promise.reject({name:'AbortError'})}});r.window.spiderShareDeal(code,'test');await new Promise(setImmediate);assert.deepEqual(r.events,['spider-share-done']);
assert.equal(await run().window.spiderCopyLink('link'),false);assert.equal(await run('','',{navigator:{clipboard:{writeText:async v=>assert.equal(v,'link')}}}).window.spiderCopyLink('link'),true);assert.equal(await run('','',{navigator:{clipboard:{writeText:()=>Promise.reject('denied')}}}).window.spiderCopyLink('link'),false);
for(const invalid of ['hello?auth=secret','x#session','x/y','a'.repeat(65)])assert.throws(()=>run().window.spiderDealLink(invalid));
console.log('Sharing: TG/VK/browser routing, public URLs, cancellation and copy fallback passed. Mock dialogs only.');

for(const klondike of ['k1_1_0123456789abcdef','k1_1_ffffffffffffffff','t1_1_0123456789abcdef','t1_1_ffffffffffffffff']){
 assert.equal(run('?deal='+klondike).window.spiderIncomingDeal(),klondike);
 assert.equal(run('?tgWebAppPlatform=ios','#tgWebAppStartParam='+klondike).window.spiderDealLink(klondike),'https://t.me/spidercardgame_bot?startapp='+klondike);
 assert.equal(run('?vk_app_id=123','#/deal/'+klondike).window.spiderIncomingDeal(),klondike);
 assert.equal(run('?vk_app_id=123','#/deal/'+klondike).window.spiderDealLink(klondike),'https://vk.com/app123#/deal/'+klondike);
}
console.log('Klondike and TriPeaks codes preserve exact u64 strings across all three adapters.');
