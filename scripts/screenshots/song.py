#!/usr/bin/env python3
"""Makes a song file for scripts/screenshots.sh: a cover and synced lyrics,
fetched from a cover URL and LRCLIB. The file stays on your Mac (only the
resulting screenshots are meant for docs/images).

usage: song.py <out.js> <key> <title> <artist> <album> <cover url> <lrclib id>
"""
import base64
import json
import re
import sys
import urllib.request


def get(url):
    req = urllib.request.Request(url, headers={'User-Agent': 'NeedleX screenshots'})
    with urllib.request.urlopen(req, timeout=30) as r:
        return r.read()


def main():
    if len(sys.argv) != 8:
        sys.exit(__doc__)
    out, key, title, artist, album, cover, lrclib = sys.argv[1:]
    art = 'data:image/jpeg;base64,' + base64.b64encode(get(cover)).decode()
    data = json.loads(get(f'https://lrclib.net/api/get/{lrclib}'))
    lines = []
    for row in (data.get('syncedLyrics') or '').splitlines():
        m = re.match(r'\[(\d+):(\d+(?:\.\d+)?)\]\s*(.*)', row)
        if m:
            lines.append({'time': round((int(m[1]) * 60 + float(m[2])) * 1000), 'text': m[3]})
    song = {
        'key': key, 'title': title, 'artist': artist, 'album': album,
        'durationMs': round(float(data.get('duration') or 200) * 1000),
        'lines': [l['text'] for l in lines],
    }
    js = (
        f'window.DemoSongOverride = Object.assign({json.dumps(song, ensure_ascii=False)}, {{\n'
        f'  art() {{ return {json.dumps(art)}; }},\n'
        f'  synced() {{ return {json.dumps(lines, ensure_ascii=False)}; }},\n'
        '});\n'
    )
    with open(out, 'w', encoding='utf-8') as f:
        f.write(js)
    print(f'{out}: {title} – {artist}, {len(lines)} lines')


if __name__ == '__main__':
    main()
