// Public deal links only: never reuse launch URLs containing platform credentials.
(() => {
 const done=()=>window.dispatchEvent?.(new Event('spider-share-done'));
 const config=()=>window.SPIDER_CONFIG||{};
 const params=()=>new URLSearchParams(location.search);
 const hashParams=()=>new URLSearchParams(location.hash.slice(1));
 const telegram=()=>hashParams().has('tgWebAppData')||params().has('tgWebAppPlatform');
 const vk=()=>params().has('vk_app_id');
 window.spiderIncomingDeal=()=>{
  const q=params(),h=hashParams();
  const direct=q.get('deal')??q.get('tgWebAppStartParam')??h.get('tgWebAppStartParam');
  if(direct!==null)return direct;
  const raw=h.get('tgWebAppData')??q.get('tgWebAppData');
  const start=new URLSearchParams(raw||'').get('start_param')??window.Telegram?.WebApp?.initDataUnsafe?.start_param;
  if(start!==undefined&&start!==null)return start;
  const route=location.hash.match(/^#\/?deal\/([^/?]+)$/);
  if(route){try{return decodeURIComponent(route[1]);}catch{return '!invalid-code';}}
  return h.get('deal');
 };
 window.spiderDealLink=code=>{
  if(!/^[a-zA-Z0-9_]{1,64}$/.test(code))throw new Error('Invalid deal code');
  const c=config();
  if(telegram()&&/^[A-Za-z0-9_]+$/.test(c.botUsername||'')){
   const app=/^[A-Za-z0-9_]+$/.test(c.appShortName||'')?'/'+c.appShortName:'';
   return `https://t.me/${c.botUsername}${app}?startapp=${encodeURIComponent(code)}`;
  }
  const vkId=c.vkAppId||params().get('vk_app_id');
  if(vk()&&/^\d+$/.test(vkId||''))return `https://vk.com/app${vkId}#/deal/${encodeURIComponent(code)}`;
  const url=new URL(c.publicUrl||'https://spider.85-137-95-37.sslip.io/');
  url.search='';url.hash='';url.searchParams.set('deal',code);return url.href;
 };
 window.spiderShareDeal=(code,text)=>{
  const url=window.spiderDealLink(code);
  if(telegram()){
   const share=`https://t.me/share/url?url=${encodeURIComponent(url)}&text=${encodeURIComponent(text)}`;
   if(window.Telegram?.WebApp?.openTelegramLink){window.Telegram.WebApp.openTelegramLink(share);done();}
   else if(window.open?.(share,'_blank','noopener'))done();
  }else if(vk()){
   const bridge=window.vkBridge?.default||window.vkBridge;
   if(bridge?.send)Promise.resolve(bridge.send('VKWebAppShare',{link:url})).then(done).catch(done);
  }else if(window.navigator?.share){
   Promise.resolve(window.navigator.share({title:code.startsWith('t1_')?'Три пика':code.startsWith('k1_')?'Косынка':'Паук',text,url})).then(done).catch(error=>{if(error?.name==='AbortError')done();});
  }
  return url;
 };
 window.spiderCopyLink=async url=>{
  if(vk()){
   const bridge=window.vkBridge?.default||window.vkBridge;
   if(bridge?.send){try{await bridge.send('VKWebAppCopyText',{text:url});return true;}catch{}}
  }
  try {if(!window.navigator?.clipboard?.writeText)return false;await window.navigator.clipboard.writeText(url);return true;}catch{return false;}
 };
})();
