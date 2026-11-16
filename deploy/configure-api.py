#!/usr/bin/env python3
"""Transfer only the runtime bot token over SSH stdin; never print it or ship .env."""
from pathlib import Path
import os, re, shlex, subprocess
root=Path(__file__).resolve().parent.parent
values={}
for line in (root/'.env').read_text().splitlines():
    if line.strip() and not line.lstrip().startswith('#') and '=' in line:
        key,value=line.split('=',1)
        parsed=shlex.split(value,comments=True)
        values[key.strip()]=parsed[0] if parsed else ''
token=values.get('TELEGRAM_BOT_TOKEN','')
if not re.fullmatch(r'[0-9]+:[A-Za-z0-9_-]+',token):
    raise SystemExit('Set a valid TELEGRAM_BOT_TOKEN in local .env before deployment')
remote=r'''import pathlib,sys,os
p=pathlib.Path('/opt/bots/bots/spider-web/.env')
p.parent.mkdir(parents=True,exist_ok=True)
lines=p.read_text().splitlines() if p.exists() else []
lines=[l for l in lines if not l.startswith('TELEGRAM_BOT_TOKEN=')]
lines.append('TELEGRAM_BOT_TOKEN='+sys.stdin.read().strip())
fd=os.open(str(p),os.O_WRONLY|os.O_CREAT|os.O_TRUNC,0o600)
os.fchmod(fd,0o600)
with os.fdopen(fd,'w') as f:f.write('\n'.join(lines)+'\n')
'''
subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=20',os.environ.get('SPIDER_SSH_HOST','root@85.137.95.37'), 'python3 -c '+shlex.quote(remote)],input=token.encode(),check=True)
print('API runtime secret configured over SSH; token not included in release artifacts.')
