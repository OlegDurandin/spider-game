"""Send only the VK app's runtime credentials via SSH stdin. Never echo values."""
from pathlib import Path
import json,os,shlex,subprocess
root=Path(__file__).resolve().parents[2]
v={}
for line in (root/'.env').read_text().splitlines():
    if '=' not in line or line.lstrip().startswith('#'):continue
    k,x=line.split('=',1);parts=shlex.split(x,comments=True);v[k.strip()]=parts[0] if parts else ''
if not v.get('VK_APP_ID','').isdigit() or int(v['VK_APP_ID'])<=0 or not v.get('VK_APP_SECRET'):raise SystemExit('Set VK_APP_ID and VK_APP_SECRET in local .env')
origins=['https://spider.62-109-4-100.sslip.io']+[x.strip() for x in v.get('VK_ALLOWED_ORIGINS','').split(',') if x.strip()]
from urllib.parse import urlsplit
for o in origins:
    u=urlsplit(o)
    if u.scheme!='https' or not u.hostname or u.username or u.password or u.path or u.query or u.fragment:raise SystemExit('VK_ALLOWED_ORIGINS must contain exact HTTPS origins without path')
values={'VK_APP_ID':v['VK_APP_ID'],'VK_APP_SECRET':v['VK_APP_SECRET'],'SPIDER_ALLOWED_ORIGINS':','.join(origins)}
remote='''from pathlib import Path
import json,sys,os
values=json.load(sys.stdin)
p=Path('/opt/bots/bots/spider-vk/.env');p.parent.mkdir(parents=True,exist_ok=True)
lines=p.read_text().splitlines() if p.exists() else []
lines=[l for l in lines if l.split('=',1)[0] not in values]
lines.extend(k+'='+json.dumps(v) for k,v in values.items())
fd=os.open(str(p),os.O_WRONLY|os.O_CREAT|os.O_TRUNC,0o600);os.fchmod(fd,0o600)
with os.fdopen(fd,'w') as f:f.write('\\n'.join(lines)+'\\n')
'''
subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=20',os.environ.get('SPIDER_VK_SSH_HOST','root@62.109.4.100'),'python3 -c '+shlex.quote(remote)],input=json.dumps(values).encode(),check=True)
print('VK runtime credentials configured via SSH stdin; no Telegram credentials transferred.')
