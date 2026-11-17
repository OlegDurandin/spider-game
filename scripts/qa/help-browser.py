"""Real headless Chrome acceptance against the release WASM; no external services.
Run: python help-browser.py /tmp/help-spider.json
Requires playwright and an installed Chrome (or set PLAYWRIGHT_CHANNEL).
"""
from pathlib import Path
from http.server import ThreadingHTTPServer,SimpleHTTPRequestHandler
from threading import Thread
from functools import partial
import os,json,sys
from playwright.sync_api import sync_playwright
ROOT=Path(__file__).resolve().parents[2]
fixture=Path(sys.argv[1]).read_text()
class Handler(SimpleHTTPRequestHandler):
    def log_message(self,*args): pass
server=ThreadingHTTPServer(('127.0.0.1',0),partial(Handler,directory=str(ROOT/'web/dist')))
Thread(target=server.serve_forever,daemon=True).start()
URL=f'http://127.0.0.1:{server.server_port}/'
BASE='spider.game.v1'
with sync_playwright() as p:
    browser=p.chromium.launch(channel=os.environ.get('PLAYWRIGHT_CHANNEL','chrome'),headless=True)
    def fresh(width=390,slot='regular',query=''):
        context=browser.new_context(viewport={'width':width,'height':844},device_scale_factor=2,reduced_motion='reduce')
        context.route('**/*',lambda route:route.continue_() if route.request.url.startswith(URL) else route.abort())
        context.add_init_script('localStorage.setItem('+json.dumps(BASE)+','+json.dumps(fixture)+');localStorage.setItem('+json.dumps(BASE+'.active')+','+json.dumps(slot)+');')
        page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)));page.set_default_timeout(10000);page.goto(URL+query);page.locator('.game').wait_for();return context,page
    def b(page,name):
        scope=page.locator('[role=dialog]') if page.locator('[role=dialog]').count() else page
        return scope.get_by_role('button',name=name,exact=True)
    def snapshot(page):return page.evaluate('JSON.stringify({...localStorage})')
    def click(page,name):b(page,name).click()
    def rules(page,title):
        page.locator('.help-panel').wait_for();assert title in page.locator('.help-heading h2').inner_text()
        assert page.locator('.help-body h3').all_text_contents()==['Цель','Как сделать ход']
        assert page.locator('.help-example .card-face').count()>=2
        assert page.locator('.help-questions details').count()>=2
    errors=[]
    # Table entry, unchanged selection/scroll/storage, all supported Back paths.
    for width in (360,390):
        ctx,page=fresh(width)
        entry=b(page,'Как играть');pause=b(page,'Пауза')
        for control in (entry,pause):
            box=control.bounding_box();assert box['width']>=44 and box['height']>=44 and box['x']+box['width']<=width,(width,box)
        assert page.evaluate('document.documentElement.scrollWidth')==width
        page.locator('.column').nth(0).locator('.card[data-card-id="6"]').click(position={'x':8,'y':5})
        assert page.locator('.selection-ring').count()==1
        board_width=page.locator('.column').nth(0).bounding_box()['width']
        old=snapshot(page)
        if width==390:page.screenshot(path=str(ROOT/'docs/how-to-play-entry-390.png'))
        for method in ('button','top','escape','browser','platform'):
            page.locator('.table-scroll').evaluate('(el)=>el.scrollTop=120')
            scroll=page.locator('.table-scroll').evaluate('(el)=>el.scrollTop')
            click(page,'Как играть');rules(page,'Паук · 2 масти')
            assert 'разные масти' in page.locator('figcaption').inner_text()
            if width==390 and method=='button':page.screenshot(path=str(ROOT/'docs/how-to-play-rules-390.png'))
            if method=='button':click(page,'Вернуться к игре')
            elif method=='top':click(page,'‹ Назад')
            elif method=='escape':page.keyboard.press('Escape')
            elif method=='browser':page.go_back()
            else:page.evaluate('window.dispatchEvent(new Event("spider-back"))')
            page.locator('.help-panel').wait_for(state='detached')
            assert page.locator('.selection-ring').count()==1,(width,method,'selection')
            assert page.locator('.table-scroll').evaluate('(el)=>el.scrollTop')==scroll,(width,method,'scroll')
            assert snapshot(page)==old,(width,method,'storage')
            assert page.locator('.column').nth(0).bounding_box()['width']==board_width
            page.wait_for_function('history.state !== "spider-help"')
        page.locator('.table-scroll').evaluate('(el)=>el.scrollTop=0')
        page.locator('.column').nth(1).locator('.card').click()
        g=json.loads(page.evaluate('localStorage.getItem("spider.game.v1")'))
        assert g['state']['moves']==1 and len(g['state']['columns'][1])==4
        print(f'PASS {width}px: entry targets, unchanged table/selection/scroll/storage, 5 Back paths, selected move completed')
        ctx.close()
    # Choice context and novice flow. New-game closing must not create a party.
    ctx,page=fresh();old=snapshot(page);click(page,'Новая игра');page.get_by_role('radio',name='Косынка',exact=True).click()
    click(page,'Как играть?');rules(page,'Косынка · По одной карте')
    assert 'красную 7♥ на чёрную 8♣' in page.locator('figcaption').inner_text()
    assert page.locator('.help-group .card').count()==2
    assert page.locator('.help-example').bounding_box()['y']+page.locator('.help-example').bounding_box()['height']<page.locator('.help-return').bounding_box()['y']
    # Text zoom remains scrollable, disclosures and return accessible.
    page.evaluate('document.documentElement.style.fontSize="200%"')
    page.locator('summary').last.click();assert page.locator('details').last.get_attribute('open') is not None
    click(page,'Назад к выбору');assert page.get_by_role('radio',name='Косынка',exact=True).get_attribute('aria-checked')=='true'
    assert snapshot(page)==old
    page.keyboard.press('Escape');assert page.locator('[role=dialog]').count()==0;assert snapshot(page)==old
    page.evaluate('document.documentElement.style.fontSize=""')
    click(page,'Новая игра');page.get_by_role('radio',name='Косынка',exact=True).click();click(page,'Как играть?');page.keyboard.press('Escape')
    assert page.locator('.new-game-panel').count()==1;click(page,'Начать игру');assert page.locator('.klondike').count()==1
    assert json.loads(page.evaluate('localStorage.getItem("spider.game.v1.klondike_regular")'))['state']['moves']==0
    click(page,'Пауза');click(page,'Действия с раскладом');click(page,'Как играть');rules(page,'Косынка');click(page,'Назад к действиям')
    assert page.locator('.app-panel h2').inner_text()=='Действия с раскладом'
    print('PASS chosen Klondike before first game; 200% text, disclosures, two-level Back, first start and legacy menu entry')
    ctx.close()

    # All bundled variants use the pending selection instead of the active table.
    ctx,page=fresh()
    click(page,'Новая игра')
    for kind,mode,title,needle in [('Паук','1 масть','1 масть','Все три карты'),('Паук','4 масти','4 масти','разные масти'),('Три пика',None,'Три пика','свободная восьмёрка')]:
        page.get_by_role('radio',name=kind,exact=True).click()
        if mode:page.get_by_role('radio',name=mode,exact=True).click()
        click(page,'Как играть?');rules(page,title);assert needle in page.locator('figcaption').inner_text()
        page.go_back();page.locator('.new-game-panel').wait_for()
        assert page.get_by_role('radio',name=kind,exact=True).get_attribute('aria-checked')=='true'
        if mode:assert page.get_by_role('radio',name=mode,exact=True).get_attribute('aria-checked')=='true'
    print('PASS one/four-suit Spider and TriPeaks choice context; browser Back retains chosen variant')
    ctx.close()
    for code,title in [('d1_20261117','Паук'),('s1_2_0123456789abcdef','Паук · 2 масти'),('t1_1_0123456789abcdef','Три пика')]:
        ctx,page=fresh(query='?deal='+code)
        click(page,'Играть');page.locator('[role=dialog]').wait_for(state='detached')
        old=snapshot(page);click(page,'Как играть');rules(page,title);click(page,'Вернуться к игре');assert snapshot(page)==old
        ctx.close()
    print('PASS table entry from daily and shared Spider / TriPeaks; saved records unchanged')
    assert not errors,errors
    browser.close()
server.shutdown()
print('PASS release WASM in real Chrome; screenshots saved at 390 CSS px / DPR 2. No physical phone or actual platform SDK used.')
