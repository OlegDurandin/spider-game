#!/usr/bin/env python3
"""Export only explicitly public settings; bot tokens are never exported."""
import json,re,shlex
from pathlib import Path
root=Path(__file__).resolve().parent.parent
allowed={'TELEGRAM_BOT_USERNAME','TELEGRAM_APP_SHORT_NAME','MINI_APP_URL','VK_APP_ID','VK_API_URL'}
settings={}
if (root/'.env').exists():
 for line in (root/'.env').read_text().splitlines():
  if '=' not in line or line.lstrip().startswith('#'):continue
  key,value=line.split('=',1)
  if key.strip() in allowed:
   parsed=shlex.split(value,comments=True);settings[key.strip()]=parsed[0] if parsed else ''
url=settings.get('MINI_APP_URL') or 'https://spider.85-137-95-37.sslip.io/'
from urllib.parse import urlsplit,urlunsplit
parts=urlsplit(url)
if parts.scheme!='https' or not parts.hostname or parts.username or parts.password:raise SystemExit('MINI_APP_URL must be a public HTTPS URL')
config={'publicUrl':urlunsplit((parts.scheme,parts.netloc,parts.path or '/', '', '')),'botUsername':settings.get('TELEGRAM_BOT_USERNAME','').lstrip('@'),'appShortName':settings.get('TELEGRAM_APP_SHORT_NAME',''),'vkAppId':settings.get('VK_APP_ID',''),'vkApiUrl':settings.get('VK_API_URL','https://spider.62-109-4-100.sslip.io')}
vk_url=urlsplit(config['vkApiUrl'])
if vk_url.scheme!='https' or not vk_url.hostname or vk_url.username or vk_url.password or vk_url.query or vk_url.fragment or vk_url.path not in ('','/'):raise SystemExit('VK_API_URL must be a public HTTPS origin')
config['vkApiUrl']=urlunsplit((vk_url.scheme,vk_url.netloc,'','',''))
if config['vkAppId'] and not config['vkAppId'].isdigit():raise SystemExit('VK_APP_ID must be numeric')
if config['botUsername'] and not re.fullmatch('[A-Za-z0-9_]+',config['botUsername']):raise SystemExit('Invalid public bot username')
(root/'web/public/config.js').write_text('window.SPIDER_CONFIG = '+json.dumps(config,ensure_ascii=True)+';\n')
print('Public share configuration prepared; no session parameters or secrets exported.')
