#!/usr/bin/env python3
"""Sync only Spider-owned files and marked sections; preserve all other work."""
import argparse
from pathlib import Path
import shutil

parser = argparse.ArgumentParser()
parser.add_argument('--repo', type=Path, required=True)
parser.add_argument('--apply', action='store_true')
args = parser.parse_args()
source = Path(__file__).resolve().parent / 'bots-server'
changes = []
for name in ['compose.yaml', 'bot.conf', '.env.example']:
    src = source / 'bots/spider-web' / name
    dest = args.repo / 'bots/spider-web' / name
    if not dest.exists() or src.read_bytes() != dest.read_bytes():
        changes.append(str(dest))
        if args.apply:
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src, dest)
for relative, fragment, begin, end in [
    ('caddy/85.137.95.37.Caddyfile', 'Caddyfile.fragment', '# BEGIN spider-game (managed from spider-game/deploy/bots-server)', '# END spider-game'),
    ('README.md', 'README.fragment.md', '<!-- BEGIN spider-game deployment -->', '<!-- END spider-game deployment -->'),
]:
    dest = args.repo / relative
    text = dest.read_text()
    part = (source / fragment).read_text().strip()
    if begin in text:
        start = text.index(begin)
        stop = text.index(end, start) + len(end)
        result = text[:start] + part + text[stop:]
    else:
        result = text.rstrip() + '\n\n' + part + '\n'
    if result != text:
        changes.append(str(dest))
        if args.apply:
            dest.write_text(result)
print(('Updated' if args.apply else 'Changes needed') + ': ' + (', '.join(changes) or 'none'))
if changes and not args.apply:
    raise SystemExit(1)
