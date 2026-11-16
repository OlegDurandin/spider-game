#!/usr/bin/env python3
"""Disposable HTTP integration: only a local QA API with a fake token."""
import concurrent.futures, copy, hashlib, hmac, json, sys, time
import urllib.error, urllib.parse, urllib.request
from pathlib import Path
BASE = 'http://127.0.0.1:8093'
TOKEN = b'123456:test-only-token'
OUT = Path('/tmp/spider-release11-api-records.json')
def auth(user):
    f = {'auth_date': str(int(time.time())), 'user': json.dumps({'id': user})}
    check = '\n'.join(f'{k}={v}' for k,v in sorted(f.items()))
    secret = hmac.new(b'WebAppData', TOKEN, hashlib.sha256).digest()
    f['hash'] = hmac.new(secret, check.encode(), hashlib.sha256).hexdigest()
    return urllib.parse.urlencode(f)
def req(user, slot, body=None):
    path = '/api/game' + ('' if slot == 'regular' else '/' + slot)
    r = urllib.request.Request(BASE + path, method='GET' if body is None else 'PUT', data=None if body is None else json.dumps(body).encode(), headers={'Authorization': 'tma ' + auth(user), 'Content-Type': 'application/json'})
    try:
        with urllib.request.urlopen(r, timeout=10) as response:
            assert 'no-store' in response.headers.get('Cache-Control', '')
            return response.status, json.load(response)
    except urllib.error.HTTPError as e:
        return e.code, None
if '--verify' in sys.argv:
    for record in json.loads(OUT.read_text()):
        assert req(record['user'], record['slot'])[1] == record['saved']
    print('PASS: every user and slot survived Docker API restart, including moves and undo history.')
    raise SystemExit
root = Path(sys.argv[1])
regular = json.loads((root / 'legacy-4.json').read_text())
day = json.loads((root / 'published-start.json').read_text())
day['deal'] = {'kind': 'daily', 'date': '20261115'}
challenge = copy.deepcopy(day)
challenge['deal'] = {'kind': 'published', 'index': 0}
slots = {'regular': regular, 'daily_20261115': day, 'challenge_p1_0': challenge}
users = [909110001, 909110002]
for user in users:
    for slot,g in slots.items():
        assert req(user, slot)[1]['game'] is None
        status, _ = req(user, slot, {'revision': 0, 'game': g})
        assert status == 200, (user, slot, status)
        saved = req(user, slot)[1]
        assert saved['game']['version'] == 3
        assert saved['game']['state'] == g['state']
        assert saved['game']['history'] == g['history']
        if slot == 'regular': assert saved['game']['deal'] is None
# Mutate one user's ordinary attempt; other users and all other slots stay unchanged.
g = req(users[0], 'regular')[1]['game']
g['history'].append(copy.deepcopy(g['state']))
row = g['state']['stock'][-10:][::-1]
g['state']['stock'] = g['state']['stock'][:-10]
for col,card in zip(g['state']['columns'], row):
    card['open'] = True
    col.append(card)
g['state']['moves'] += 1
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    statuses = list(pool.map(lambda _: req(users[0], 'regular', {'revision': 1, 'game': g})[0], range(2)))
assert sorted(statuses) == [200,409], statuses
assert req(users[1], 'regular')[1]['game']['state'] == regular['state']
assert req(users[0], 'daily_20261115', {'revision': 1, 'game': challenge})[0] == 422
assert req(users[0], 'bad-slot')[0] == 400
records = [{'user':u,'slot':s,'saved':req(u,s)[1]} for u in users for s in slots]
assert records[0]['saved']['game'] == g
OUT.write_text(json.dumps(records))
print('PASS: two authenticated users × three independent slots; v2 migration, move/history persistence, concurrent CAS, mismatched IDs and invalid slots.')
