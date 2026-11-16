#!/usr/bin/env python3
"""HTTP checks only against an isolated localhost Docker API and a fake bot token."""
import concurrent.futures, copy, hashlib, hmac, json, sys, time
import urllib.error, urllib.parse, urllib.request
from pathlib import Path
BASE='http://127.0.0.1:8097'
TOKEN=b'123456:test-only-token'
RECORDS=Path('/tmp/spider12-api-records.json')
def auth(user):
    f={'auth_date':str(int(time.time())),'user':json.dumps({'id':user})}
    check='\n'.join(f'{k}={v}' for k,v in sorted(f.items()))
    secret=hmac.new(b'WebAppData',TOKEN,hashlib.sha256).digest()
    f['hash']=hmac.new(secret,check.encode(),hashlib.sha256).hexdigest()
    return urllib.parse.urlencode(f)
def request(user,slot,body=None,signature=None):
    path='/api/game'+('' if slot=='regular' else '/'+slot)
    r=urllib.request.Request(BASE+path,method='GET' if body is None else 'PUT',data=None if body is None else json.dumps(body).encode(),headers={'Authorization':'tma '+(signature or auth(user)),'Content-Type':'application/json'})
    try:
        with urllib.request.urlopen(r,timeout=10) as res:
            assert 'no-store' in res.headers.get('Cache-Control','')
            return res.status,json.load(res)
    except urllib.error.HTTPError as e:return e.code,None
if '--verify' in sys.argv:
    for record in json.loads(RECORDS.read_text()):
        assert request(record['user'],record['slot'])[1]==record['saved']
    print('PASS: both users and all Spider/Klondike/day/challenge states, exact stock order and undo history survived Docker API restart.')
    raise SystemExit
root=Path(sys.argv[1]);load=lambda name:json.loads((root/name).read_text())
slots={'regular':load('spider-legacy4.json'),'klondike_regular':load('klondike-redealt.json'),'daily_20261115':load('compact-day.json'),'challenge_k1_1_0123456789abcdef':load('klondike-start.json')}
users=[909120001,909120002]
for user in users:
    for slot,g in slots.items():
        assert request(user,slot)[1]['game'] is None
        body=copy.deepcopy(g)
        if user==users[1] and slot=='klondike_regular':body=load('klondike-ordinary.json')
        code,_=request(user,slot,{'revision':0,'game':body});assert code==200,(slot,code)
        saved=request(user,slot)[1]['game']
        assert saved['state']==body['state'] and saved['history']==body['history']
        assert saved['kind']==('klondike' if slot.startswith(('klondike','challenge_k')) else 'spider')
assert request(users[0],'regular',signature=auth(users[0]).replace('909120001','909120002'))[0]==401
before_b=request(users[1],'klondike_regular')[1]
g=request(users[0],'klondike_regular')[1]['game'];g['history'].append(copy.deepcopy(g['state']));g['history']=g['history'][-100:]
card=g['state']['stock'].pop();card['open']=True;g['state']['waste'].append(card);g['state']['moves']+=1
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    results=list(pool.map(lambda _:request(users[0],'klondike_regular',{'revision':1,'game':g})[0],range(2)))
assert sorted(results)==[200,409],results
assert request(users[1],'klondike_regular')[1]==before_b
assert request(users[0],'regular',{'revision':1,'game':g})[0]==422
bad=copy.deepcopy(g);bad['kind']='unknown'
assert request(users[0],'klondike_regular',{'revision':2,'game':bad})[0]==422
bad=copy.deepcopy(g);bad['state']['stock'][0]=bad['state']['stock'][1]
assert request(users[0],'klondike_regular',{'revision':2,'game':bad})[0]==422
assert request(users[0],'challenge_k1_1_0123456789abcdef',{'revision':1,'game':load('klondike-ordinary.json')})[0]==422
assert request(users[0],'klondike_regular')[1]['game']==g
records=[{'user':u,'slot':s,'saved':request(u,s)[1]} for u in users for s in slots]
RECORDS.write_text(json.dumps(records))
print('PASS: 2 users × 4 slots; legacy four-suit Spider migration; Klondike redeal/history/exact u64; concurrent CAS; wrong kind, duplicate cards and mismatched invitation rejected.')
