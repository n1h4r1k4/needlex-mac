// NeedleX – the cover as an object on your desktop: a turntable, a record
// sleeve or the cover in an aura of its own colours. Lives in the wallpaper
// window just above the desktop icons; drag it anywhere, click to play / pause.
// Browser preview: desk.html?look=sleeve
const DEMO = !window.__TAURI__ ? new URLSearchParams(location.search) : null;
const { invoke, listen } = DEMO ? demoBridge(DEMO) : { invoke: window.__TAURI__.core.invoke, listen: window.__TAURI__.event.listen };

function demoBridge(q) {
  const handlers = {};
  const c = document.createElement('canvas');
  c.width = c.height = 300;
  const g = c.getContext('2d');
  const gr = g.createLinearGradient(0, 0, 300, 300);
  gr.addColorStop(0, '#f4d1c4'); gr.addColorStop(0.5, '#a3475a'); gr.addColorStop(1, '#1b1020');
  g.fillStyle = gr; g.fillRect(0, 0, 300, 300);
  g.fillStyle = 'rgba(255,255,255,.85)'; g.beginPath(); g.arc(170, 130, 60, 0, 7); g.fill();
  document.body.style.background = 'linear-gradient(160deg, #6b8fb5, #c9a27a)';
  setTimeout(() => {
    handlers.settings?.({ payload: { wallpaper: q.get('look') || 'turntable' } });
    handlers['media:track']?.({ payload: { key: 'demo', title: 'National Treasures', artist: 'Drake', album: '', art: c.toDataURL(), source: 'demo', sourceName: 'Demo' } });
    handlers['media:state']?.({ payload: { active: true, playing: q.get('paused') == null } });
  }, 100);
  return {
    invoke: async () => null,
    listen: (ev, fn) => { handlers[ev] = fn; },
  };
}

const $ = (s) => document.querySelector(s);
const $$ = (s) => [...document.querySelectorAll(s)];
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
const LOOKS = ['turntable', 'sleeve', 'aura'];
const obj = $('#obj');

const S = {
  look: 'turntable',
  track: null,
  hires: {},
  active: false,
  playing: false,
  toggleUntil: 0,
  shown: false,
  pos: loadPos(),
  level: 0,
  levelAt: 0,
  gotLevel: false,
  smooth: 0,
  raf: 0,
  lastT: 0,
};

function loadPos() {
  try {
    const p = JSON.parse(localStorage.getItem('deskPos'));
    if (p && isFinite(p.x) && isFinite(p.y)) return p;
  } catch {}
  return { x: 0.5, y: 0.5 };
}
function savePos() { try { localStorage.setItem('deskPos', JSON.stringify(S.pos)); } catch {} }

// ------------------------------------------------------------ placing

// Centre of the object at S.pos (fractions of the screen), kept clear of the
// menu bar and the Dock.
function place() {
  const w = obj.offsetWidth, h = obj.offsetHeight;
  const W = innerWidth, H = innerHeight;
  const x = clamp(S.pos.x * W, w / 2 + 12, Math.max(w / 2 + 12, W - w / 2 - 12));
  const y = clamp(S.pos.y * H, h / 2 + 44, Math.max(h / 2 + 44, H - h / 2 - 96));
  obj.style.left = `${Math.round(x - w / 2)}px`;
  obj.style.top = `${Math.round(y - h / 2)}px`;
  report();
}

// tells NeedleX where the object is, so only that spot takes the mouse
function report() {
  let r = { x: 0, y: 0, w: 0, h: 0 };
  if (S.shown) {
    const b = obj.getBoundingClientRect();
    r = { x: b.left, y: b.top, w: b.width, h: b.height };
  }
  invoke('set_wall_hit', r).catch(() => {});
}

function setLook(look) {
  if (!LOOKS.includes(look)) return;
  S.look = look;
  for (const l of LOOKS) obj.classList.toggle(`look-${l}`, l === look);
  place();
  frameSoon();
}

// ------------------------------------------------------------ art

const PLACEHOLDER = 'data:image/svg+xml;utf8,' + encodeURIComponent(
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#3a3a44"/><stop offset="1" stop-color="#16161b"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/><path d="M42 30v32.5a9 9 0 1 0 5 8V43l20-5v19.5a9 9 0 1 0 5 8V24z" fill="#ffffff" fill-opacity=".55"/></svg>`);

function setArt(src) {
  $$('img.art').forEach((i) => { i.src = src; });
  const img = new Image();
  img.onload = () => lampColour(img);
  img.src = src;
}

// a bright, saturated colour of the cover for the record player's lamp
function lampColour(img) {
  try {
    const N = 24;
    const c = document.createElement('canvas');
    c.width = c.height = N;
    const x = c.getContext('2d', { willReadFrequently: true });
    x.drawImage(img, 0, 0, N, N);
    const d = x.getImageData(0, 0, N, N).data;
    let r = 0, g = 0, b = 0, w = 0;
    for (let i = 0; i < d.length; i += 4) {
      const mx = Math.max(d[i], d[i + 1], d[i + 2]), mn = Math.min(d[i], d[i + 1], d[i + 2]);
      const k = ((mx - mn) / 255) ** 2 * (mx / 255) + 0.001;
      r += d[i] * k; g += d[i + 1] * k; b += d[i + 2] * k; w += k;
    }
    let c3 = [r / w, g / w, b / w];
    const m = Math.max(...c3) || 1;
    c3 = c3.map((v) => Math.round(clamp((v / m) * 255, 60, 255)));
    document.documentElement.style.setProperty('--light', c3.join(', '));
  } catch {}
}

// players hand out small thumbnails; fetch the full-size cover
async function loadHires(t) {
  try {
    const url = await invoke('hires_art', { title: t.title, artist: t.artist || '', album: t.album || '' });
    if (!url || !S.track || S.track.key !== t.key) return;
    S.hires[t.key] = url;
    setArt(url);
  } catch {}
}

// ------------------------------------------------------------ state

// On the desktop while a song is loaded (the record stops when it is paused);
// gone when the player is.
function update() {
  const playing = S.active && S.playing;
  document.body.classList.toggle('playing', playing);
  const keep = S.active && !!S.track;
  if (keep !== S.shown) {
    S.shown = keep;
    obj.classList.toggle('show', keep);
    report();
    setTimeout(report, 1200); // after the entrance transition
  }
  frameSoon();
}

listen('media:track', (e) => {
  const t = e.payload;
  if (!t || !t.key) { S.active = false; update(); return; }
  const isNew = !S.track || S.track.key !== t.key;
  const artChanged = isNew || S.track.art !== t.art;
  S.track = t;
  $('.caption .title').textContent = t.title;
  $('.caption .artist').textContent = t.artist || t.album || t.sourceName || '';
  if (artChanged && !S.hires[t.key]) setArt(t.art || PLACEHOLDER);
  if (isNew) loadHires(t);
  update();
});

listen('media:state', (e) => {
  const p = e.payload || {};
  S.active = !!p.active;
  if (S.active && performance.now() > S.toggleUntil) S.playing = !!p.playing;
  update();
});

listen('audio:level', (e) => {
  S.level = Math.max(S.level * 0.6, e.payload);
  S.levelAt = performance.now();
  S.gotLevel = true;
});

function applySettings(s) {
  if (s && s.wallpaper) setLook(s.wallpaper);
}
invoke('get_settings').then(applySettings).catch(() => {});
listen('settings', (e) => applySettings(e.payload));

// ------------------------------------------------------------ aura pulse

function frameSoon() {
  if (!S.raf && S.look === 'aura' && S.shown && S.playing) S.raf = requestAnimationFrame(frame);
}
function frame(t) {
  if (S.look !== 'aura' || !S.shown || !(S.active && S.playing)) {
    S.raf = 0;
    S.smooth = 0;
    obj.style.setProperty('--level', '0');
    return;
  }
  S.raf = requestAnimationFrame(frame);
  if (t - S.lastT < 30) return; // ~30 fps is plenty here
  S.lastT = t;
  if (t - S.levelAt > 120) S.level *= 0.85;
  let target = Math.min(1, Math.sqrt(S.level) * 1.1);
  if (!S.gotLevel) target = 0.4 + 0.2 * Math.sin(t / 320);
  S.smooth += (target - S.smooth) * (target > S.smooth ? 0.5 : 0.12);
  obj.style.setProperty('--level', S.smooth.toFixed(3));
}

// ------------------------------------------------------------ drag / click

let drag = null;
obj.addEventListener('mousedown', (e) => {
  if (e.button !== 0) return;
  e.preventDefault();
  const b = obj.getBoundingClientRect();
  drag = { sx: e.clientX, sy: e.clientY, cx: b.left + b.width / 2, cy: b.top + b.height / 2, moved: false };
});
addEventListener('mousemove', (e) => {
  if (!drag) return;
  const dx = e.clientX - drag.sx, dy = e.clientY - drag.sy;
  if (!drag.moved && Math.hypot(dx, dy) < 5) return;
  if (!drag.moved) { drag.moved = true; obj.classList.add('dragging'); }
  S.pos = { x: (drag.cx + dx) / innerWidth, y: (drag.cy + dy) / innerHeight };
  place();
});
addEventListener('mouseup', () => {
  if (!drag) return;
  const d = drag;
  drag = null;
  obj.classList.remove('dragging');
  if (d.moved) {
    // remember where it ended up (after keeping it on the screen)
    const b = obj.getBoundingClientRect();
    S.pos = { x: (b.left + b.width / 2) / innerWidth, y: (b.top + b.height / 2) / innerHeight };
    savePos();
    report();
  } else if (S.track) {
    S.playing = !S.playing;
    S.toggleUntil = performance.now() + 1200;
    update();
    invoke('media_control', { action: 'toggle' }).catch(() => {});
  }
});

addEventListener('resize', place);

setArt(PLACEHOLDER);
setLook(S.look);
invoke('media_resend').catch(() => {});
