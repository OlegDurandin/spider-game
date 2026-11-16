#!/usr/bin/env python3
"""Black-box API checks against a disposable Docker instance with a fake bot token."""
import concurrent.futures, copy, hashlib, hmac, json, os, sys, time, urllib.error, urllib.parse, urllib.request
from pathlib import Path
BASE=os.environ.get('SPIDER_TEST_API','http://127.0.0.1:8083')
assert BASE in ('http://127.0.0.1:8083','http://127.0.0.1:8093'), 'Use a disposable local QA API'
TOKEN=b'123456:test-only-token'
def auth(user):
    fields={'auth_date':str(int(time.time())),'user':json.dumps({'id':user,'first_name':'QA'},separators=(',',':'))}
    check='\n'.join(f'{k}={v}' for k,v in sorted(fields.items()))
    secret=hmac.new(b'WebAppData',TOKEN,hashlib.sha256).digest()
    fields['hash']=hmac.new(secret,check.encode(),hashlib.sha256).hexdigest()
    return urllib.parse.urlencode(fields)
def request(user,body=None,authorization=None):
    req=urllib.request.Request(BASE+'/api/game', method='PUT' if body is not None else 'GET', data=json.dumps(body).encode() if body is not None else None,headers={'Authorization':'tma '+(authorization or auth(user)),'Content-Type':'application/json'})
    try:
        with urllib.request.urlopen(req,timeout=10) as r:
            assert 'no-store' in r.headers.get('Cache-Control','')
            return r.status,json.load(r)
    except urllib.error.HTTPError as e:return e.code,None
def game(mode,offset):
    deck=[{'id':(i+offset)%104,'open':False} for i in range(104)]
    columns=[]
    for n in [6]*4+[5]*6:
        col,deck=deck[:n],deck[n:];col[-1]['open']=True;columns.append(col)
    return {'kind':'spider','variant':'spider','version':3,'deal':None,'state':{'mode':mode,'columns':columns,'stock':deck,'completed':[],'moves':0},'history':[]}
def deal(g):
    g=copy.deepcopy(g);g['history'].append(copy.deepcopy(g['state']))
    row=g['state']['stock'][:10];del g['state']['stock'][:10]
    for col,card in zip(g['state']['columns'],row):card['open']=True;col.append(card)
    g['state']['moves']+=1
    return g
if len(sys.argv)>1 and sys.argv[1]=='--verify':
    records=json.loads(Path(sys.argv[2]).read_text())
    for user,saved in records.items():assert request(int(user))[1]==saved
    print('Persistent user games, modes, moves and undo history survived API restart.')
    raise SystemExit
users=[909000001,909000002]
ga,gb=game('four',0),game('two',9)
for u in users:assert request(u)[1]['game'] is None
assert request(users[0],{'revision':0,'game':ga})[0]==200
assert request(users[1])[1]['game'] is None
assert request(users[1],{'revision':0,'game':gb})[0]==200
assert request(users[0])[1]['game']==ga
assert request(users[1])[1]['game']==gb
next_game=deal(ga)
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    statuses=list(pool.map(lambda _:request(users[0],{'revision':1,'game':next_game})[0],range(2)))
assert sorted(statuses)==[200,409],statuses
assert request(users[0],authorization=auth(users[0]).replace('909000001','909000002'))[0]==401
assert request(users[0],{'revision':0,'game':gb})[0]==409
bad=copy.deepcopy(ga);bad['state']['stock'][0]['id']=bad['state']['stock'][1]['id']
assert request(users[0],{'revision':2,'game':bad})[0]==422
records={str(u):request(u)[1] for u in users}
assert records[str(users[1])]['game']==gb
assert records[str(users[0])]['game']==next_game
Path('/tmp/spider-account-records.json').write_text(json.dumps(records))
print('HTTP integration: two users isolated; forged identity rejected; concurrent/stale writes protected; invalid cards rejected.')
