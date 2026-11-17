"""Real VK client/API integration with test-only credentials, two HTTP origins and isolated SQLite."""
from pathlib import Path
from http.server import ThreadingHTTPServer,SimpleHTTPRequestHandler
from threading import Thread
from functools import partial
import os,sys,json,time,hmac,hashlib,base64,urllib.parse,urllib.request,tempfile,subprocess
from playwright.sync_api import sync_playwright
ROOT=Path(__file__).resolve().parents[2]
SECRET='vk-test-secret'
def query(user=777):
    v={'vk_app_id':'123','vk_user_id':str(user),'vk_ts':str(int(time.time())),'vk_ref':'other/с пробелом&x=1'}
    body='&'.join(k+'='+urllib.parse.quote(v[k],safe="~!*'()-._") for k in sorted(v))
    v['sign']=base64.urlsafe_b64encode(hmac.new(SECRET.encode(),body.encode(),hashlib.sha256).digest()).decode().rstrip('=')
    return urllib.parse.urlencode(v)
class Handler(SimpleHTTPRequestHandler):
    def log_message(self,*a):pass
    def do_GET(self):
        if self.path.split('?')[0]=='/config.js':
            b=b'window.SPIDER_CONFIG={vkAppId:"123",vkApiUrl:"http://127.0.0.1:8082",publicUrl:"https://example.invalid/"};'
            self.send_response(200);self.send_header('Content-Type','application/javascript');self.end_headers();self.wfile.write(b)
        else:super().do_GET()
web=ThreadingHTTPServer(('127.0.0.1',0),partial(Handler,directory=str(ROOT/'web/dist')))
Thread(target=web.serve_forever,daemon=True).start();URL=f'http://127.0.0.1:{web.server_port}/'
with tempfile.TemporaryDirectory(prefix='spider-vk-qa-') as td:
    env={**os.environ,'TELEGRAM_BOT_TOKEN':'','VK_APP_ID':'123','VK_APP_SECRET':SECRET,'SPIDER_DB':td+'/game.sqlite3','SPIDER_ALLOWED_ORIGINS':URL.rstrip('/')}
    log=open(td+'/api.log','w+');api=subprocess.Popen([str(ROOT/'target/debug/spider-server')],env=env,stdout=log,stderr=log)
    local=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    try:
        for _ in range(400):
            try:local.open('http://127.0.0.1:8082/healthz',timeout=1).close();break
            except Exception:time.sleep(.05)
        else:
            log.seek(0);raise RuntimeError('API did not start: '+log.read())
        with sync_playwright() as p:
            browser=p.chromium.launch(channel='chrome',headless=True)
            context=browser.new_context(viewport={'width':390,'height':844},device_scale_factor=2)
            context.route('**/*',lambda r:r.continue_() if r.request.url.startswith((URL,'http://127.0.0.1:8082/')) else r.abort())
            page=context.new_page();page.set_default_timeout(15000);errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
            page.goto(URL+'?'+query());page.locator('.game').wait_for();assert 'Аккаунт VK' in page.locator('.account-status').inner_text()
            def settled(slot):
                page.wait_for_function('(key)=>{const x=JSON.parse(localStorage.getItem(key)||"null");return x && !x.dirty}',arg='spider.vk.777.v1'+('' if slot=='regular' else '.'+slot))
            def state(slot):return page.evaluate('(k)=>JSON.parse(localStorage.getItem(k)).game', 'spider.vk.777.v1'+('' if slot=='regular' else '.'+slot))
            settled('regular');page.locator('.deal-button').click();settled('regular');assert state('regular')['state']['moves']==1
            for name,slot,stock in [('Косынка','klondike_regular','.k-stock'),('Три пика','tripeaks_regular','.t-stock')]:
                page.get_by_role('button',name='Новая игра',exact=True).click();page.get_by_role('radio',name=name,exact=True).click();page.get_by_role('button',name='Начать игру',exact=True).click()
                page.locator(stock).click();settled(slot);before=state(slot);assert before['state']['moves']==1
                page.reload();page.locator(stock).wait_for();settled(slot);assert state(slot)==before
            page.get_by_role('button',name='Как играть',exact=True).click();assert 'Три пика' in page.locator('.help-heading h2').inner_text();page.get_by_role('button',name='Вернуться к игре',exact=True).click()
            # No VK parameters or proof are persisted as part of account cache.
            stored=page.evaluate('JSON.stringify({...localStorage})');assert SECRET not in stored and 'sign=' not in stored
            assert not page.evaluate('localStorage.getItem("spider.game.v1")')
            # A different signed user cannot see the saved parties.
            page.goto(URL+'?'+query(778));page.locator('.game').wait_for();assert 'Аккаунт VK' in page.locator('.account-status').inner_text()
            page.wait_for_function('localStorage.getItem("spider.vk.778.v1")!==null')
            assert page.evaluate('JSON.parse(localStorage.getItem("spider.vk.778.v1")).game.state.moves')==0
            # Invalid or unsigned launch is rejected; never falls back to a guest game.
            for q in ['vk_app_id=123',query().replace('vk_user_id=777','vk_user_id=779')]:
                page.goto(URL+'?'+q);page.locator('#startup').wait_for();page.wait_for_function('!document.querySelector("#startup").textContent.includes("Загружаем")')
                assert page.locator('.game').count()==0
            assert not errors,errors
            browser.close()
        print('PASS VK release WASM + real API: cross-origin auth, all three saves/reloads, distinct users, no guest fallback, no persisted proofs, rules.')
    finally:
        api.terminate();api.wait(timeout=5);web.shutdown()
