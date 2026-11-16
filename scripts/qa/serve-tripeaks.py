"""Local acceptance host: exposes built assets and synthetic fixtures only."""
from http.server import ThreadingHTTPServer, SimpleHTTPRequestHandler
from pathlib import Path
import sys
root=Path(__file__).resolve().parents[2]
fixtures=Path(sys.argv[1] if len(sys.argv)>1 else '/tmp/tripeaks-qa/fixtures')
class Handler(SimpleHTTPRequestHandler):
    def translate_path(self,path):
        path=path.split('?',1)[0].split('#',1)[0]
        if path=='/qa': return str(root/'scripts/qa/tripeaks.html')
        if path.startswith('/fixtures/'): return str(fixtures/Path(path).name)
        return str(root/'web/dist'/ (path.lstrip('/') or 'index.html'))
ThreadingHTTPServer(('127.0.0.1',8795),Handler).serve_forever()
