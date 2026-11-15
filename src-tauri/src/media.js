// NeedleX media poller (JavaScript for Automation, run by /usr/bin/osascript).
// Prints one JSON line to stdout with what Spotify and Apple Music are playing:
// every 0.5 s while music plays, every second while paused, every 2 s when
// neither player is open. The song's details are only asked for again when the
// song changes (each question is an Apple Event the player has to answer).
// Apps that are not running are never launched. Exits on its own when NeedleX
// goes away (the write to the closed pipe fails).
ObjC.import('Foundation');

const out = $.NSFileHandle.fileHandleWithStandardOutput;
function emit(o) {
  const s = $.NSString.alloc.initWithUTF8String(JSON.stringify(o) + '\n');
  out.writeData(s.dataUsingEncoding($.NSUTF8StringEncoding));
}

const NOT_ALLOWED = -1743; // errAEEventNotPermitted: user said no in Privacy & Security → Automation
const cache = {};
function app(name) {
  if (!(name in cache)) {
    try { cache[name] = Application(name); } catch (e) { cache[name] = null; }
  }
  return cache[name];
}
function running(a) { try { return !!a && a.running(); } catch (e) { return false; } }
function str(f) { try { const v = f(); return v == null ? '' : String(v); } catch (e) { return ''; } }
function num(f) { try { const v = Number(f()); return isFinite(v) ? v : 0; } catch (e) { return 0; } }

// per player: the last song's id and its details
const known = {};
function details(player, id, read) {
  const k = known[player];
  if (id && k && k.id === id) return k.d;
  const d = read();
  known[player] = { id, d };
  return d;
}

function spotify(denied) {
  const sp = app('Spotify');
  if (!running(sp)) return null;
  let st;
  try { st = String(sp.playerState()); } catch (e) { if (e.errorNumber === NOT_ALLOWED) denied.push('Spotify'); return null; }
  if (!st || st === 'stopped') return null;
  const t = sp.currentTrack;
  const d = details('spotify', str(() => t.id()), () => ({
    title: str(() => t.name()), artist: str(() => t.artist()), album: str(() => t.album()),
    durationMs: num(() => t.duration()), artUrl: str(() => t.artworkUrl()),
  }));
  if (!d.title) return null;
  return {
    app: 'spotify', name: 'Spotify', title: d.title,
    artist: d.artist, album: d.album,
    durationMs: d.durationMs,
    positionMs: num(() => sp.playerPosition()) * 1000,
    playing: st === 'playing',
    artUrl: d.artUrl,
    canSeek: true,
  };
}

function music(denied) {
  const mu = app('Music');
  if (!running(mu)) return null;
  let st;
  try { st = String(mu.playerState()); } catch (e) { if (e.errorNumber === NOT_ALLOWED) denied.push('Music'); return null; }
  if (!st || st === 'stopped') return null;
  const t = mu.currentTrack;
  const d = details('music', str(() => t.persistentID()), () => ({
    title: str(() => t.name()), artist: str(() => t.artist()), album: str(() => t.album()),
    durationMs: num(() => t.duration()) * 1000,
  }));
  let title = d.title, artist = d.artist, album = d.album;
  // internet radio: the song is in the stream title ("Artist - Title")
  const stream = str(() => mu.currentStreamTitle());
  if (stream) {
    const parts = stream.split(' - ');
    if (parts.length >= 2) { album = title; artist = parts[0]; title = parts.slice(1).join(' - '); }
    else { album = title; title = stream; }
  }
  if (!title) return null;
  return {
    app: 'music', name: 'Apple Music', title, artist, album,
    durationMs: stream ? 0 : d.durationMs,
    positionMs: num(() => mu.playerPosition()) * 1000,
    playing: st === 'playing' || st === 'fast forwarding' || st === 'rewinding',
    artUrl: '',
    canSeek: !stream,
  };
}

function run() {
  for (;;) {
    const sessions = [], denied = [];
    for (const f of [spotify, music]) {
      try { const s = f(denied); if (s) sessions.push(s); } catch (e) {}
    }
    emit({ sessions, denied });
    delay(sessions.some((x) => x.playing) ? 0.5 : sessions.length ? 1 : 2);
  }
}
