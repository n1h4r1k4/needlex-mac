// NeedleX – full-screen "now playing": cover view → click → word-by-word lyrics
const DEMO = !window.__TAURI__ ? new URLSearchParams(location.search) : null;
const { invoke, listen } = DEMO ? demoBridge(DEMO) : { invoke: window.__TAURI__.core.invoke, listen: window.__TAURI__.event.listen };
// Desktop wallpaper: the same view drawn on the desktop behind every window
// (the "wall" window), and on the lock screen (the "lock" window, with the
// clock). In a browser: full.html?wall=1 or ?lock=1
const LABEL = (() => {
  try { if (window.__TAURI__) return window.__TAURI__.window.getCurrentWindow().label; } catch {}
  const q = new URLSearchParams(location.search);
  return q.has('lock') ? 'lock' : q.has('wall') ? 'wall' : 'full';
})();
const LOCK = LABEL === 'lock';
const WALL = LABEL === 'wall' || LOCK;

// Lets the page be previewed in a normal browser: full.html?mode=lyrics&style=stars&t=9000
function demoBridge(q) {
  const handlers = {};
  const c = document.createElement('canvas');
  c.width = c.height = 300;
  const g = c.getContext('2d');
  const gr = g.createLinearGradient(0, 0, 300, 300);
  gr.addColorStop(0, '#f4d1c4'); gr.addColorStop(0.5, '#a3475a'); gr.addColorStop(1, '#1b1020');
  g.fillStyle = gr; g.fillRect(0, 0, 300, 300);
  g.fillStyle = 'rgba(255,255,255,.85)'; g.beginPath(); g.arc(170, 130, 60, 0, 7); g.fill();
  const text = ['First day back in the game since', 'We been hotel to hotel, money on my brain', 'Go and tell them I\'m', '', 'Oh they know the way'];
  const lyrics = { found: true, instrumental: false, plain: null, synced: text.map((t, i) => ({ time: i * 4000, text: t })) };
  if (q.get('snap')) {
    const st = document.createElement('style');
    st.textContent = '*,*::before,*::after{transition-duration:0s!important;transition-delay:0s!important}';
    document.head.appendChild(st);
    setInterval(() => document.getAnimations().forEach((a) => {
      if (a.effect && a.effect.getTiming().iterations !== Infinity && a.playState === 'running') a.finish();
    }), 30);
  }
  setTimeout(() => {
    const t0 = Number(q.get('t') || 6000);
    handlers['media:track']?.({ payload: { key: 'demo', title: 'National Treasures', artist: 'Drake', album: '', art: q.get('art') || c.toDataURL(), source: 'demo', sourceName: 'Demo' } });
    handlers['media:state']?.({ payload: { active: true, playing: true, positionMs: t0, durationMs: 200000, hasTimeline: true, canNext: true, canPrev: true, canSeek: true } });
    handlers['full:open']?.({});
    if (q.get('style')) setStyle(q.get('style'));
    if (q.get('glitter')) applySettings({ glitter: q.get('glitter') });
    if (q.get('bg') || q.get('cover')) applySettings({ lyricStyle: q.get('style') || 'words', lyricBg: q.get('bg') || 'simple', lyricCover: q.get('cover') === '1' });
    if (q.get('mode') === 'lyrics') setTimeout(() => setMode('lyrics'), 1300);
  }, 100);
  return {
    invoke: async (cmd) => (cmd === 'fetch_lyrics' ? lyrics : null),
    listen: (ev, fn) => { const prev = handlers[ev]; handlers[ev] = prev ? (e) => { prev(e); fn(e); } : fn; },
  };
}

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const lsGet = (k) => { try { return localStorage.getItem(k); } catch { return null; } };
const lsSet = (k, v) => { try { localStorage.setItem(k, v); } catch {} };
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));

const stage = $('#stage');
const artBox = $('.art-box');
const artCanvas = $('#art');
const heroCanvas = $('#bg-words .hero');
const card = $('.card');

const S = {
  open: false,
  closing: false,
  track: null,
  play: { active: false, playing: false, positionMs: 0, durationMs: 0, hasTimeline: false, canSeek: false },
  playAt: performance.now(),
  localPos: 0,
  level: 0,
  lastLevelAt: 0,
  gotLevel: false,
  mode: 'cover',
  style: 'words', // the lyrics effect (tray → Lyrics effect, or the Aa button)
  img: null,
  lyricsRes: null,
  lyricsLoading: false,
  lines: [],
  lyrT: 0,
  li: -2,
  wi: -2,
  boxAnim: null,
  busy: false,
  seekUntil: 0,
  toggleUntil: 0,
  idleT: 0,
  raf: 0,
  hires: {},
  pixStart: -1e9,
  glitch: 0,
  slowLevel: 0,
  glUntil: 0,
  settings: { glow: true },
};

// ------------------------------------------------------------ time

function pos() {
  const p = S.play;
  if (!p.hasTimeline) return S.localPos;
  const v = p.positionMs + (p.playing ? performance.now() - S.playAt : 0);
  return clamp(v, 0, p.durationMs);
}
function fmt(ms) {
  if (!isFinite(ms) || ms < 0) ms = 0;
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

// ------------------------------------------------------------ random (stable per line)

function hashStr(str) {
  let h = 2166136261;
  for (let i = 0; i < str.length; i++) { h ^= str.charCodeAt(i); h = Math.imul(h, 16777619); }
  return h >>> 0;
}
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a |= 0; a = (a + 0x6D2B79F5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// ------------------------------------------------------------ colours

function rgb2hsl(r, g, b) {
  r /= 255; g /= 255; b /= 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  let h = 0, s = 0; const l = (max + min) / 2;
  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    if (max === r) h = (g - b) / d + (g < b ? 6 : 0);
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
    h /= 6;
  }
  return [h, s, l];
}
function hsl2rgb(h, s, l) {
  if (s === 0) return [l * 255, l * 255, l * 255];
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s, p = 2 * l - q;
  const f = (t) => {
    if (t < 0) t += 1; if (t > 1) t -= 1;
    if (t < 1 / 6) return p + (q - p) * 6 * t;
    if (t < 1 / 2) return q;
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
    return p;
  };
  return [f(h + 1 / 3) * 255, f(h) * 255, f(h - 1 / 3) * 255];
}
const rgbStr = (c) => c.map((v) => Math.round(v)).join(', ');

// up to 6 distinct, glowing colours of the cover (for album-coloured glitter)
// the vivid tone of the cover, for when it has few colours of its own
function accentOf(d) {
  let vr = 0, vg = 0, vb = 0, vw = 0;
  for (let i = 0; i < d.length; i += 4) {
    const [, s, l] = rgb2hsl(d[i], d[i + 1], d[i + 2]);
    const w = s * s * Math.max(0, 1 - Math.abs(l - 0.55) * 1.8) + 0.0005;
    vr += d[i] * w; vg += d[i + 1] * w; vb += d[i + 2] * w; vw += w;
  }
  const [h, s] = rgb2hsl(vr / vw, vg / vw, vb / vw);
  return hsl2rgb(h, Math.max(s, 0.55), 0.66);
}

function albumColours(d) {
  const bins = Array.from({ length: 12 }, () => ({ r: 0, g: 0, b: 0, w: 0 }));
  for (let i = 0; i < d.length; i += 4) {
    const [h, s, l] = rgb2hsl(d[i], d[i + 1], d[i + 2]);
    if (s < 0.18 || l < 0.12 || l > 0.95) continue;
    const bn = bins[Math.floor(h * 12) % 12];
    const w = s * (1 - Math.abs(l - 0.55));
    bn.r += d[i] * w; bn.g += d[i + 1] * w; bn.b += d[i + 2] * w; bn.w += w;
  }
  const out = bins.filter((b) => b.w > 0.6).sort((a, b) => b.w - a.w).slice(0, 6).map((b) => {
    const [h, s] = rgb2hsl(b.r / b.w, b.g / b.w, b.b / b.w);
    return hsl2rgb(h, Math.max(0.6, s), 0.68);
  });
  out.push([255, 255, 255]);                       // a little white sparkle always helps
  return out;
}

function applyPalette(img) {
  const N = 36;
  const c = document.createElement('canvas');
  c.width = c.height = N;
  const x = c.getContext('2d', { willReadFrequently: true });
  x.drawImage(img, 0, 0, N, N);
  const d = x.getImageData(0, 0, N, N).data;
  let lr = 0, lg = 0, lb = 0, lw = 0;  // light
  let vr = 0, vg = 0, vb = 0, vw = 0;  // vivid
  let ar = 0, ag = 0, ab = 0;          // average
  for (let i = 0; i < d.length; i += 4) {
    const r = d[i], g = d[i + 1], b = d[i + 2];
    const [, s, l] = rgb2hsl(r, g, b);
    ar += r; ag += g; ab += b;
    if (l > 0.42) { const w = l * l * (0.35 + s); lr += r * w; lg += g * w; lb += b * w; lw += w; }
    const vwt = s * s * Math.max(0, 1 - Math.abs(l - 0.55) * 1.8) + 0.0005;
    vr += r * vwt; vg += g * vwt; vb += b * vwt; vw += vwt;
  }
  const n = d.length / 4;
  const avg = [ar / n, ag / n, ab / n];
  let light = lw > 0 ? [lr / lw, lg / lw, lb / lw] : avg;
  let [h, s, l] = rgb2hsl(...light);
  light = hsl2rgb(h, Math.min(s, 0.5), clamp(l, 0.74, 0.9));
  [h, s, l] = rgb2hsl(...avg);
  const dark = hsl2rgb(h, Math.min(s, 0.5), Math.min(l, 0.14));
  [h, s, l] = rgb2hsl(vr / vw, vg / vw, vb / vw);
  const accent = hsl2rgb(h, vw / n < 0.02 ? Math.min(s, 0.1) : Math.max(s, 0.55), clamp(l, 0.6, 0.74));
  const root = document.documentElement.style;
  root.setProperty('--light', rgbStr(light));
  S.lightRGB = light;
  const cols = albumColours(d);
  if (window.Glitter) Glitter.setPalette(cols);
  // beat glow: the cover's own colours (without the white sparkle); a grey
  // cover still gets its accent and light tones
  if (window.EdgeLight) {
    const vivid = cols.slice(0, -1);
    EdgeLight.setPalette(vivid.length >= 2 ? vivid : [...vivid, accentOf(d), light].slice(0, 3));
  }
  root.setProperty('--dark', rgbStr(dark));
  root.setProperty('--accent', rgbStr(accent));
}

// ------------------------------------------------------------ art drawing

function drawCrop(ctx, img, W, H, smooth = true) {
  const iw = img.naturalWidth || img.width, ih = img.naturalHeight || img.height;
  const s = Math.min(iw, ih);
  ctx.imageSmoothingEnabled = smooth;
  ctx.imageSmoothingQuality = 'high';
  ctx.clearRect(0, 0, W, H);
  ctx.drawImage(img, (iw - s) / 2, (ih - s) / 2, s, s, 0, 0, W, H);
}

// The new cover "resolves" from big pixels to sharp, like in the reel.
function pixelReveal(canvas, img) {
  const tok = (canvas._tok = (canvas._tok || 0) + 1);
  const ctx = canvas.getContext('2d');
  const steps = [4, 7, 11, 18, 30, 52, 96];
  const tmp = document.createElement('canvas');
  const tc = tmp.getContext('2d');
  let i = 0;
  (function step() {
    if (canvas._tok !== tok) return;
    if (i >= steps.length) { drawCrop(ctx, img, canvas.width, canvas.height); return; }
    const n = steps[i++];
    tmp.width = tmp.height = n;
    drawCrop(tc, img, n, n);
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(tmp, 0, 0, canvas.width, canvas.height);
    setTimeout(step, 70);
  })();
}

function sizeCanvases() {
  const dpr = window.devicePixelRatio || 1;
  const a = Math.round(artBox.clientWidth * dpr) || 512;
  if (artCanvas.width !== a) { artCanvas.width = artCanvas.height = a; }
  const hsz = Math.round(heroCanvas.clientWidth * dpr) || 900;
  if (heroCanvas.width !== hsz) { heroCanvas.width = heroCanvas.height = hsz; }
  if (S.img) {
    drawCrop(artCanvas.getContext('2d'), S.img, artCanvas.width, artCanvas.height);
    drawCrop(heroCanvas.getContext('2d'), S.img, heroCanvas.width, heroCanvas.height);
  }
}

// zoom-blur stack for the "stars" background
const zoom = $('#bg-stars .zoom');
const zoomImgs = [];
(function buildZoom() {
  const base = document.createElement('img');
  base.className = 'base';
  zoom.appendChild(base);
  zoomImgs.push(base);
  for (let i = 1; i <= 7; i++) {
    const im = document.createElement('img');
    im.className = 'streak';
    im.style.transform = `scale(${1 + i * 0.032})`;
    im.style.opacity = (0.62 - i * 0.07).toFixed(2);
    zoom.appendChild(im);
    zoomImgs.push(im);
  }
})();

// When only a small cover exists (players hand Windows ~300 px thumbnails and
// some songs are in no catalogue), upscale it in 2x high-quality steps and
// apply a light unsharp mask, so it stays crisp on big / high-DPI screens.
function enhanced(img) {
  const iw = img.naturalWidth, ih = img.naturalHeight;
  const s = Math.min(iw, ih);
  const dpr = window.devicePixelRatio || 1;
  const screenPx = Math.max(screen.width, screen.height) * dpr;
  const target = Math.round(Math.min(2000, Math.max(900, screenPx * 0.55)));
  if (s >= target * 0.85) return img;
  let c = document.createElement('canvas');
  c.width = c.height = s;
  c.getContext('2d').drawImage(img, (iw - s) / 2, (ih - s) / 2, s, s, 0, 0, s, s);
  while (c.width < target) {
    const n = Math.min(target, c.width * 2);
    const d = document.createElement('canvas');
    d.width = d.height = n;
    const x = d.getContext('2d');
    x.imageSmoothingEnabled = true;
    x.imageSmoothingQuality = 'high';
    x.drawImage(c, 0, 0, n, n);
    c = d;
  }
  const n = c.width;
  const x = c.getContext('2d', { willReadFrequently: true });
  const b = document.createElement('canvas');
  b.width = b.height = n;
  const bx = b.getContext('2d', { willReadFrequently: true });
  bx.filter = `blur(${Math.max(1, (n / s) * 0.6).toFixed(2)}px)`;
  bx.drawImage(c, 0, 0);
  const o = x.getImageData(0, 0, n, n), bl = bx.getImageData(0, 0, n, n);
  const od = o.data, bd = bl.data, k = 0.6;
  for (let i = 0; i < od.length; i += 4) {
    od[i] = od[i] + k * (od[i] - bd[i]);
    od[i + 1] = od[i + 1] + k * (od[i + 1] - bd[i + 1]);
    od[i + 2] = od[i + 2] + k * (od[i + 2] - bd[i + 2]);
  }
  x.putImageData(o, 0, 0);
  return c;
}

function setArt(src, animate) {
  const img = new Image();
  img.decoding = 'async';
  img.onload = () => {
    S.img = enhanced(img);
    applyPalette(img);
    zoomImgs.forEach((i) => { i.src = src; });
    $('#bg-words .wash').src = src;
    $$('#bg-aura .wash, #bg-big .wash, #bg-glass .wash, #bg-fog .wash, #lock-record img').forEach((i) => { i.src = src; });
    $('.vinyl img').src = src;
    $('#thumb').src = src;
    if (window.FlowBG) FlowBG.setImage(S.img);
    if (!animate) { artCanvas._tok = (artCanvas._tok || 0) + 1; heroCanvas._tok = (heroCanvas._tok || 0) + 1; }
    sizeCanvases();
    if (animate) {
      pixelReveal(artCanvas, S.img);
      pixelReveal(heroCanvas, S.img);
      S.pixStart = performance.now();
      S.glitch = 1;
    }
  };
  img.src = src;
}

// The players only hand Windows a small thumbnail; fetch the full-size cover.
async function loadHiresArt(t) {
  try {
    const url = await invoke('hires_art', { title: t.title, artist: t.artist || '', album: t.album || '' });
    if (!url || !S.track || S.track.key !== t.key) return;
    S.hires[t.key] = url;
    setArt(url, false);
  } catch {}
}

const PLACEHOLDER = 'data:image/svg+xml;utf8,' + encodeURIComponent(
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#3a3a44"/><stop offset="1" stop-color="#16161b"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/><path d="M42 30v32.5a9 9 0 1 0 5 8V43l20-5v19.5a9 9 0 1 0 5 8V24z" fill="#ffffff" fill-opacity=".55"/></svg>`);

// ------------------------------------------------------------ track / state

function setTrack(t) {
  if (!t || !t.key) return; // keep showing the last song
  const isNew = !S.track || S.track.key !== t.key;
  const artChanged = isNew || S.track.art !== t.art;
  S.track = t;
  $('.meta .title').textContent = t.title;
  $('.meta .artist').textContent = t.artist || t.album || t.sourceName;
  if (artChanged && !S.hires[t.key]) setArt(t.art || PLACEHOLDER, S.open);
  if (isNew) {
    loadHiresArt(t);
    S.localPos = 0;
    resetLyrics();
    clearTimeout(S.lyrT);
    S.lyrT = setTimeout(() => loadLyrics(t, 0), 650);
  }
}

function onState(p) {
  const now = performance.now();
  const predicted = pos();
  const prev = S.play;
  const next = { ...p };
  if (!next.active) return;
  if (now < S.toggleUntil) next.playing = prev.playing;
  if (next.hasTimeline && prev.hasTimeline) {
    const diff = Math.abs(predicted - next.positionMs);
    if ((next.playing && prev.playing && diff < 450) || (now < S.seekUntil && diff > 450)) next.positionMs = predicted;
  }
  S.play = next;
  S.playAt = now;
  stage.classList.toggle('playing', next.playing);
}

// ------------------------------------------------------------ lyrics data

// LRCLIB gives line timings; spread each line's words across the line so the
// lyrics can appear one word at a time.
function buildLines(synced) {
  return synced.map((ln, i) => {
    const words = ln.text.split(/\s+/).filter(Boolean);
    const nextT = i + 1 < synced.length ? synced[i + 1].time : ln.time + Math.max(3000, words.length * 500);
    const span = Math.min(nextT - ln.time, Math.max(900, words.length * 620)) * 0.86;
    const weights = words.map((w) => w.length + 2.5);
    const total = weights.reduce((a, b) => a + b, 0) || 1;
    let acc = 0;
    return {
      time: ln.time,
      words: words.map((w, k) => { const t = ln.time + (span * acc) / total; acc += weights[k]; return { w, t }; }),
    };
  });
}

function resetLyrics() {
  S.lines = [];
  S.lyricsRes = null;
  S.lyricsLoading = true;
  S.li = S.wi = -2;
  Stars.reset();
  Words.reset();
  Term.reset();
  Lines.reset();
  updateMsg();
}

async function loadLyrics(t, attempt) {
  if (!S.track || S.track.key !== t.key) return;
  try {
    const res = await invoke('fetch_lyrics', {
      title: t.title, artist: t.artist || '', durationMs: S.play.hasTimeline ? S.play.durationMs : 0,
    });
    if (!S.track || S.track.key !== t.key) return;
    S.lyricsRes = res;
    S.lines = buildLines(res.synced || []);
  } catch {
    if (attempt < 2) { S.lyrT = setTimeout(() => loadLyrics(t, attempt + 1), 4000); return; }
  }
  S.lyricsLoading = false;
  S.li = S.wi = -2;
  updateMsg();
}

function updateMsg() {
  const el = $('#lyr-msg');
  let msg = '';
  if (!S.lyricsLoading && !S.lines.length) {
    const r = S.lyricsRes;
    msg = r && r.instrumental ? '♪  instrumental  ♪' : r && r.plain ? 'no synced lyrics for this song' : 'no lyrics for this song';
  }
  el.textContent = msg;
  el.classList.toggle('show', !!msg);
}

function lineIndex(p) {
  const L = S.lines;
  let lo = 0, hi = L.length - 1, ans = -1;
  while (lo <= hi) {
    const m = (lo + hi) >> 1;
    if (L[m].time <= p) { ans = m; lo = m + 1; } else hi = m - 1;
  }
  return ans;
}

// ------------------------------------------------------------ style: stars

const SPARK = `<svg class="spark" viewBox="-50 -50 100 100"><path d="M0-50C1.6-10 10-1.6 50 0 10 1.6 1.6 10 0 50-1.6 10-10 1.6-50 0-10-1.6-1.6-10 0-50Z" fill="#fff"/><path d="M0-16C.8-3 3-.8 16 0 3 .8.8 3 0 16-.8 3-3 .8-16 0-3-.8-.8-3 0-16Z" fill="#fff" transform="rotate(45)"/><circle r="3.2" fill="#fff"/></svg>`;
const SVGNS = 'http://www.w3.org/2000/svg';

const Stars = {
  root: $('#stars'),
  consts: [],
  lastAnchor: -1,
  reset() { this.root.innerHTML = ''; this.consts = []; },

  update(li, wi, jumped) {
    if (jumped) this.reset();
    if (li < 0) return;
    const line = S.lines[li];
    let c = this.consts.find((o) => o.li === li);
    if (!c) {
      // a single constellation at a time
      const keep = 1;
      for (const o of this.consts) {
        o.age++;
        if (o.age < keep) o.el.classList.add('old');
        else { o.el.classList.add('gone'); setTimeout(() => o.el.remove(), 1500); }
      }
      this.consts = this.consts.filter((o) => o.age < keep);
      if (!line.words.length) return;
      c = this.build(li, line);
      this.consts.push(c);
    }
    c.show(wi);
  },

  build(li, line) {
    const W = innerWidth, H = innerHeight;
    const r = rng(hashStr((S.track ? S.track.key : '') + '#' + li));
    const anchors = [[0.3, 0.6], [0.67, 0.6], [0.47, 0.72], [0.71, 0.45], [0.31, 0.44], [0.55, 0.55], [0.24, 0.72], [0.77, 0.73]];
    let ai = Math.floor(r() * anchors.length);
    if (ai === this.lastAnchor) ai = (ai + 3) % anchors.length;
    this.lastAnchor = ai;
    const n = line.words.length;
    const step = Math.min(W, H) * (n > 9 ? 0.12 : n > 5 ? 0.15 : 0.19);
    // where the stars may go: a box in the middle of the screen (under the
    // clock in the full view, between the clock and the password field on
    // the lock screen)
    const box = LOCK ? [0.18, 0.82, 0.32, 0.76] : WALL ? [0.16, 0.84, 0.24, 0.76] : [0.14, 0.86, 0.32, 0.82];
    if (sideCover()) { box[0] = 0.46; box[1] = 0.92; }
    const [bx0, bx1, by0, by1] = [box[0] * W, box[1] * W, box[2] * H, box[3] * H];
    let x = W / 2, y = (by0 + by1) / 2;
    let ang = r() * Math.PI * 2;
    const pts = [];
    for (let k = 0; k < n; k++) {
      if (k > 0) {
        let nx, ny, tries = 0;
        do {
          ang += (r() - 0.5) * 2.4;
          const d = step * (0.7 + r() * 0.6);
          nx = x + Math.cos(ang) * d;
          ny = y + Math.sin(ang) * d * 0.8;
          tries++;
        } while ((nx < bx0 || nx > bx1 || ny < by0 || ny > by1
          || pts.some((p) => Math.hypot(p.x - nx, p.y - ny) < step * 0.6)) && tries < 30);
        x = clamp(nx, bx0, bx1);
        y = clamp(ny, by0, by1);
      }
      pts.push({ x, y });
    }
    {
      // centre the whole constellation in its box
      const xs = pts.map((p) => p.x), ys = pts.map((p) => p.y);
      const dx = (bx0 + bx1) / 2 - (Math.min(...xs) + Math.max(...xs)) / 2;
      const dy = (by0 + by1) / 2 - (Math.min(...ys) + Math.max(...ys)) / 2;
      for (const p of pts) { p.x += dx; p.y += dy; }
    }

    const el = document.createElement('div');
    el.className = 'const';
    const svg = document.createElementNS(SVGNS, 'svg');
    el.appendChild(svg);

    const nodes = pts.map((p, k) => {
      const nd = document.createElement('div');
      nd.className = 'node';
      nd.style.left = p.x + 'px';
      nd.style.top = p.y + 'px';
      nd.innerHTML = `${SPARK}<span class="ring"></span><span class="dot"></span><span class="lbl"></span>`;
      nd.querySelector('.lbl').textContent = line.words[k].w;
      el.appendChild(nd);
      return nd;
    });

    const edges = [];
    const addEdge = (a, b) => {
      const ln = document.createElementNS(SVGNS, 'line');
      ln.setAttribute('x1', pts[a].x); ln.setAttribute('y1', pts[a].y);
      ln.setAttribute('x2', pts[b].x); ln.setAttribute('y2', pts[b].y);
      ln.setAttribute('pathLength', '1');
      svg.appendChild(ln);
      edges.push({ to: b, el: ln });
    };
    for (let k = 1; k < n; k++) {
      addEdge(k - 1, k);
      if (k >= 2 && r() < 0.42) addEdge(Math.floor(r() * (k - 1)), k);
    }

    this.root.appendChild(el);
    const words = line.words;
    return {
      li, el, age: 0, shown: -1,
      show(wi) {
        for (let k = this.shown + 1; k <= wi; k++) {
          nodes[k].classList.add('on');
          for (const e of edges) if (e.to === k) e.el.classList.add('on');
        }
        this.shown = Math.max(this.shown, wi);
        nodes.forEach((nd, k) => {
          nd.classList.toggle('cur', k === wi);
          const w = words[k].w.replace(/[^\p{L}\p{N}]/gu, '');
          nd.classList.toggle('major', k < wi && (w.length >= 5 || k === n - 1));
        });
      },
    };
  },
};

// ------------------------------------------------------------ word icons

// A few words get a little picture next to them (Big Words, Aura), drawn in
// the same colour as the word.
const ICONS = {
  bulb: 'M12 2a7 7 0 0 0-4 12.74V17a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1v-2.26A7 7 0 0 0 12 2zM9 20h6v1a2 2 0 0 1-2 2h-2a2 2 0 0 1-2-2z',
  note: 'M10 3v11.56A4 4 0 1 0 12 18V8.5l6 1.6V5.1z',
  heart: 'M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54z',
  star: 'M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z',
  flame: 'M13.5.67s.74 2.65.74 4.8c0 2.06-1.35 3.73-3.41 3.73-2.07 0-3.63-1.67-3.63-3.73l.03-.36C5.21 7.51 4 10.62 4 14c0 4.42 3.58 8 8 8s8-3.58 8-8C20 8.61 17.41 3.8 13.5.67z',
  moon: 'M12 3a9 9 0 1 0 9 9c0-.46-.04-.92-.1-1.36a5.39 5.39 0 0 1-4.4 2.26 5.4 5.4 0 0 1-3.14-9.8C12.92 3.04 12.46 3 12 3z',
  sun: 'M12 7a5 5 0 1 0 0 10 5 5 0 0 0 0-10zM11 1h2v3h-2zm0 19h2v3h-2zM1 11h3v2H1zm19 0h3v2h-3zM4.22 5.64l1.42-1.42 2.12 2.12-1.42 1.42zm12.02 12.02l1.42-1.42 2.12 2.12-1.42 1.42zM4.22 18.36l2.12-2.12 1.42 1.42-2.12 2.12zM16.24 6.34l2.12-2.12 1.42 1.42-2.12 2.12z',
  drop: 'M12 2.7S5 10.3 5 15a7 7 0 0 0 14 0c0-4.7-7-12.3-7-12.3z',
};
const ICON_WORDS = {};
for (const [icon, words] of Object.entries({
  bulb: 'light lights lit bulb',
  note: 'song songs music sing singing sang melody tune radio record',
  heart: 'love loved lover loving heart hearts',
  star: 'star stars starlight',
  flame: 'fire burn burning burned flame flames hot',
  moon: 'moon night nights tonight midnight',
  sun: 'sun sunshine sunlight sunrise sunset summer',
  drop: 'rain raining tear tears cry crying ocean sea',
})) for (const w of words.split(' ')) ICON_WORDS[w] = icon;
function iconFor(word) {
  const n = ICON_WORDS[word.toLowerCase().replace(/[^a-z]/g, '')];
  return n ? `<svg class="ico" viewBox="0 0 24 24" aria-hidden="true"><path d="${ICONS[n]}" fill="currentColor"/></svg>` : '';
}

// ------------------------------------------------------------ style: Big Words / Lens / Aura

// The previous word small above, the current word big in the middle, the
// coming word waiting below (Lens: in a label). Big Words and Aura add a
// little picture to some words; Lens tilts the words a bit.
const Words = {
  root: $('#words'),
  els: [],
  chip: null,
  li: -2,
  wi: -1,
  reset() { this.root.innerHTML = ''; this.els = []; this.chip = null; this.li = -2; this.wi = -1; },
  geo() {
    // centred on the desktop; under the clock in full screen and on the lock
    // screen (where the bottom stays clear for the password field)
    const dy = LOCK ? 0.05 : WALL ? 0 : 0.06;
    return { cx: sideCover() ? 0.66 : 0.5, prev: 0.33 + dy, cur: 0.5 + dy, next: 0.68 + dy, tilt: S.style === 'lens' ? 5 : 1.4 };
  },
  place(e) {
    e.el.style.transform = `translate(${e.x * innerWidth}px, ${e.y * innerHeight}px) translate(-50%, -50%) rotate(${e.rot}deg)`;
  },
  // very long words are shrunk to fit the screen
  fit(e) {
    e.el.style.fontSize = '';
    const max = innerWidth * (sideCover() ? 0.5 : 0.9);
    const w = e.el.scrollWidth;
    if (w > max) e.el.style.fontSize = `${(parseFloat(getComputedStyle(e.el).fontSize) * max) / w}px`;
  },
  drop(e) { e.el.classList.add('gone'); setTimeout(() => e.el.remove(), 800); },
  pop(e) {
    e.el.classList.add('enter');
    requestAnimationFrame(() => requestAnimationFrame(() => e.el.classList.remove('enter')));
  },
  flash() {                       // beat: the big word lights up for a moment
    const cur = this.els.find((x) => x.role === 'cur');
    if (!cur) return;
    cur.el.classList.add('beat');
    setTimeout(() => cur.el.classList.remove('beat'), 140);
  },
  update(li, wi, jumped) {
    if (jumped) this.reset();
    const line = li >= 0 ? S.lines[li] : null;
    if (!line || !line.words.length || wi < 0) {
      if (line && !line.words.length) { this.els.forEach((e) => this.drop(e)); this.els = []; }
      this.li = li; this.wi = -1;
      this.makeChip(li, -1);
      return;
    }
    const from = !jumped && this.li === li ? this.wi + 1 : Math.max(0, wi - 1);
    for (let k = from; k <= wi; k++) this.push(line, li, k);
    this.li = li;
    this.wi = wi;
    this.makeChip(li, wi);
  },
  push(line, li, k) {
    const g = this.geo();
    const r = rng(hashStr(li + ':' + k));
    const cx = g.cx + (r() - 0.5) * 0.04;
    for (const e of this.els) {
      e.age++;
      if (e.age === 1) {
        e.role = 'prev';
        e.el.classList.remove('cur', 'enter', 'beat');
        e.el.classList.add('prev');
        e.el.querySelector('.ico')?.remove();
        e.el.style.fontSize = '';
        Object.assign(e, { x: cx + (r() - 0.5) * 0.03, y: g.prev, rot: (r() - 0.5) * g.tilt * 0.6 });
        this.place(e);
      } else this.drop(e);
    }
    this.els = this.els.filter((e) => e.age < 2);
    let e, fresh = false;
    if (this.chip && this.chip.li === li && this.chip.k === k) {
      e = this.chip.e;                // the waiting word grows into the big word
      this.chip = null;
      e.el.classList.remove('chip');
    } else {
      if (this.chip) { this.drop(this.chip.e); this.chip = null; }
      const el = document.createElement('div');
      el.className = 'word';
      el.textContent = line.words[k].w;
      this.root.appendChild(el);
      e = { el };
      fresh = true;
    }
    if (S.style !== 'lens') e.el.insertAdjacentHTML('beforeend', iconFor(line.words[k].w));
    Object.assign(e, { age: 0, role: 'cur', x: cx, y: g.cur, rot: (r() - 0.5) * g.tilt });
    e.el.classList.add('cur');
    this.place(e);
    this.fit(e);
    if (fresh) this.pop(e);
    this.els.push(e);
  },
  makeChip(li, wi) {
    let nl = li, nk = wi + 1;
    if (nl < 0 || !S.lines[nl] || nk >= S.lines[nl].words.length) {
      nl = li + 1; nk = 0;
      const next = S.lines[nl];
      if (!next || !next.words.length || next.time - pos() > 2600) nl = -1;
    }
    if (nl < 0) { if (this.chip) { this.drop(this.chip.e); this.chip = null; } return; }
    if (this.chip && this.chip.li === nl && this.chip.k === nk) return;
    if (this.chip) this.drop(this.chip.e);
    const g = this.geo();
    const cur = this.els.find((x) => x.role === 'cur');
    const r = rng(hashStr('chip' + nl + ':' + nk));
    const el = document.createElement('div');
    el.className = 'word chip';
    el.textContent = S.lines[nl].words[nk].w;
    this.root.appendChild(el);
    const e = { el, role: 'chip', x: (cur ? cur.x : g.cx) + (r() - 0.5) * 0.05, y: g.next, rot: (r() - 0.5) * g.tilt };
    this.place(e);
    this.pop(e);
    this.chip = { li: nl, k: nk, e };
  },
};

// ------------------------------------------------------------ style: Terminal

// The line being sung is typed word by word in a monospace font with a red
// block cursor; the two lines before it fade above, words joined by dots.
const Term = {
  root: $('#term'),
  li: -2,
  row: null,
  reset() { this.root.innerHTML = ''; this.li = -2; this.row = null; },
  update(li, wi, jumped) {
    if (jumped || li !== this.li) this.build(li);
    if (!this.row) return;
    this.row.querySelectorAll('.w').forEach((w, k) => w.classList.toggle('on', k <= wi));
  },
  build(li) {
    this.li = li;
    this.root.innerHTML = '';
    this.row = null;
    const text = (k) => (S.lines[k] ? S.lines[k].words.map((w) => w.w.replace(/[.,!?;:]+$/, '')).join('.') : '');
    for (const [k, cls] of [[li - 2, 'o2'], [li - 1, 'o1']]) {
      const t = k >= 0 ? text(k) : '';
      if (!t) continue;
      const d = document.createElement('div');
      d.className = `row ${cls}`;
      d.textContent = t;
      this.root.appendChild(d);
    }
    const line = li >= 0 ? S.lines[li] : null;
    const row = document.createElement('div');
    row.className = 'row cur';
    if (line) for (const w of line.words) {
      const sp = document.createElement('span');
      sp.className = 'w';
      sp.textContent = w.w + ' ';
      row.appendChild(sp);
    }
    row.insertAdjacentHTML('beforeend', '<span class="cursor"></span>');
    this.root.appendChild(row);
    this.row = row;
  },
};

// ------------------------------------------------------------ style: scrolling lines

// The whole song next to the cover, the line being sung bright, scrolling along.
const Lines = {
  inner: $('#lines .inner'),
  els: [],
  built: null,
  li: -2,
  reset() { this.inner.innerHTML = ''; this.els = []; this.built = null; this.li = -2; },
  build() {
    this.inner.innerHTML = '';
    this.els = S.lines.map((ln) => {
      const d = document.createElement('div');
      d.className = 'ln';
      d.textContent = ln.words.map((w) => w.w).join(' ') || '♪';
      this.inner.appendChild(d);
      return d;
    });
    this.built = S.lines;
    this.li = -2;
  },
  update(li) {
    if (this.built !== S.lines) this.build();
    if (li === this.li) return;
    this.li = li;
    this.els.forEach((d, k) => { d.classList.toggle('cur', k === li); d.classList.toggle('past', k < li); });
    const d = this.els[Math.max(0, li)];
    if (d) this.inner.style.transform = `translateY(${-(d.offsetTop + d.offsetHeight / 2)}px)`;
  },
};

// ------------------------------------------------------------ modes

// Needle's five lyric effects, plus the whole song scrolling next to the cover
const STYLES = ['words', 'lens', 'stars', 'terminal', 'aura', 'lines'];

// The cover stays next to the lyrics (scrolling lines, or "Show album cover");
// otherwise it blows up into the background as the lyrics start.
function sideCover() { return S.style === 'lines' || !!S.settings.lyricCover; }
const BGS = ['simple', 'glass', 'liquid', 'foggy'];

function setStyle(style) {
  if (!STYLES.includes(style)) style = 'words';
  const wasSide = stage.classList.contains('side');
  S.style = style;
  for (const k of STYLES) stage.classList.toggle(`style-${k}`, k === style);
  const bg = BGS.includes(S.settings.lyricBg) ? S.settings.lyricBg : 'simple';
  for (const k of BGS) stage.classList.toggle(`bg-${k}`, k === bg);
  stage.classList.toggle('side', sideCover());
  Stars.reset();
  Words.reset();
  Term.reset();
  Lines.reset();
  S.li = S.wi = -2;
  // switching between "cover aside" and "cover blown up": redo the transition
  if (S.mode === 'lyrics' && wasSide !== sideCover()) {
    if (S.boxAnim) { S.boxAnim.cancel(); S.boxAnim = null; }
    card.getAnimations().forEach((a) => a.cancel());
    S.mode = 'cover';
    S.busy = false;
    stage.classList.replace('mode-lyrics', 'mode-cover');
    $('#cover').style.opacity = '1';
    setMode('lyrics');
  }
}

function fullTransform() {
  // transform that blows the cover up to fill the screen
  artBox.style.transform = 'none';
  const r = artBox.getBoundingClientRect();
  const W = innerWidth, H = innerHeight;
  const scale = Math.max(W / r.width, H / r.height) * 1.04;
  const dx = W / 2 - (r.left + r.width / 2);
  const dy = H / 2 - (r.top + r.height / 2);
  artBox.style.transform = '';
  return `translate(${dx}px, ${dy}px) scale(${scale})`;
}

async function setMode(m) {
  if (S.mode === m || S.busy) return;
  if (sideCover()) {                // the cover just moves aside for the lyrics
    S.mode = m;
    stage.classList.toggle('mode-lyrics', m === 'lyrics');
    stage.classList.toggle('mode-cover', m !== 'lyrics');
    $('#cover').style.opacity = '1';
    S.li = S.wi = -2;
    return;
  }
  S.busy = true;
  if (S.boxAnim) { S.boxAnim.cancel(); S.boxAnim = null; }
  const full = fullTransform();
  const radius = getComputedStyle(artBox).borderRadius;
  if (m === 'lyrics') {
    card.animate([{ opacity: 1, transform: 'none' }, { opacity: 0, transform: 'translateY(3vh) scale(.96)' }],
      { duration: 350, easing: 'ease-in', fill: 'forwards' });
    S.boxAnim = artBox.animate(
      [{ transform: 'none', borderRadius: radius }, { transform: full, borderRadius: '0px' }],
      { duration: 820, easing: 'cubic-bezier(.65,0,.2,1)', fill: 'forwards' });
    setTimeout(() => {
      S.mode = 'lyrics';
      stage.classList.replace('mode-cover', 'mode-lyrics');
      $('#cover').style.opacity = '0'; // the blown-up cover dissolves into the lyrics background
      S.li = S.wi = -2;
    }, 420);
    await S.boxAnim.finished.catch(() => {});
  } else {
    S.mode = 'cover';
    stage.classList.replace('mode-lyrics', 'mode-cover');
    $('#cover').style.opacity = '1';
    S.boxAnim = artBox.animate(
      [{ transform: full, borderRadius: '0px' }, { transform: 'none', borderRadius: radius }],
      { duration: 900, easing: 'cubic-bezier(.2,1.08,.3,1)', fill: 'forwards' });
    card.getAnimations().forEach((a) => a.cancel());
    card.animate([{ opacity: 0, transform: 'translateY(3vh) scale(.96)' }, { opacity: 1, transform: 'none' }],
      { duration: 650, delay: 380, easing: 'cubic-bezier(.2,1.1,.3,1)', fill: 'backwards' });
    await S.boxAnim.finished.catch(() => {});
  }
  S.busy = false;
}

function enter(slow) {
  S.closing = false;
  S.busy = false;
  stage.getAnimations().forEach((a) => a.cancel());
  card.getAnimations().forEach((a) => a.cancel());
  if (S.boxAnim) { S.boxAnim.cancel(); S.boxAnim = null; }
  S.mode = 'cover';
  stage.classList.remove('mode-lyrics');
  stage.classList.add('mode-cover');
  $('#cover').style.opacity = '1';
  sizeCanvases();
  updateClock();

  stage.animate([{ opacity: 0 }, { opacity: 1 }], { duration: slow ? 1800 : 500, easing: 'ease-out', fill: 'forwards' });
  // the cover grows out of the notch at the top of the screen
  if (!LOCK) artBox.animate(
    [{ transform: 'translateY(-36vh) scale(.1)', opacity: 0.4 }, { transform: 'none', opacity: 1 }],
    { duration: 1000, easing: 'cubic-bezier(.2,1.1,.3,1)' });
  $('#clock').animate([{ opacity: 0, transform: 'translateY(-2vh)' }, { opacity: 1, transform: 'none' }],
    { duration: 800, delay: 120, easing: 'cubic-bezier(.2,1,.3,1)', fill: 'backwards' });
  card.animate([{ opacity: 0, transform: 'translateY(4vh) scale(.94)' }, { opacity: 1, transform: 'none' }],
    { duration: 800, delay: 320, easing: 'cubic-bezier(.2,1.1,.3,1)', fill: 'backwards' });

  const hint = $('#hint');
  const shown = Number(lsGet('hintShown') || 0);
  if (!WALL && shown < 3) {
    lsSet('hintShown', String(shown + 1));
    hint.classList.add('show');
    setTimeout(() => hint.classList.remove('show'), 3500);
  }
}

async function exit() {
  if (S.closing || !S.open) return;
  S.closing = true;
  const a = stage.animate([{ opacity: 1, transform: 'none' }, { opacity: 0, transform: 'scale(1.035)' }],
    { duration: 420, easing: 'cubic-bezier(.4,0,.6,1)', fill: 'forwards' });
  await a.finished.catch(() => {});
  S.open = false;
  await invoke('close_full');
  stage.getAnimations().forEach((x) => x.cancel());
  stage.style.transform = '';
  S.closing = false;
}

function open(opts) {
  const auto = !!(opts && opts.auto);
  S.open = true;
  if (window.FlowBG) FlowBG.resize();
  if (window.EdgeLight) EdgeLight.resize();
  if (window.Glitter) Glitter.resize();
  enter(auto);
  requestAnimationFrame(sizeGlass);
  // macOS hands the keyboard to the page as the window opens and WebKit may
  // focus the first button; drop that so Enter / Space only do their shortcut
  const unfocus = () => { const a = document.activeElement; if (a && a !== document.body) a.blur(); };
  unfocus(); setTimeout(unfocus, 120); setTimeout(unfocus, 400);
  // opened by the idle timer: drift into the lyrics on its own, like a screensaver
  clearTimeout(S.autoT);
  if (auto) S.autoT = setTimeout(() => { if (S.open && S.mode === 'cover' && S.lines.length) setMode('lyrics'); }, 6000);
  if (!S.raf) S.raf = requestAnimationFrame(frame);
}

// ------------------------------------------------------------ input

document.addEventListener('click', (e) => {
  if (!S.open || S.closing || WALL) return;
  if (e.target.closest('button, .card, #corner')) return;
  const W = innerWidth;
  const onSide = e.clientX < W * 0.15 || e.clientX > W * 0.85;
  if (onSide) {
    if (e.detail >= 3) exit(); // triple-click on either side exits full screen
    return;
  }
  if (e.detail > 1) return;
  if (S.mode === 'cover') {
    if (e.target.closest('.art-box')) setMode('lyrics');
  } else {
    setMode('cover');
  }
});

document.addEventListener('keydown', (e) => {
  if (!S.open || WALL) return;
  if (e.key === 'Escape') exit();
  else if (e.key === ' ') { e.preventDefault(); control('toggle'); }
  else if (e.key === 'ArrowRight') control('next');
  else if (e.key === 'ArrowLeft') control('prev');
  else if (e.key === 'Enter') setMode(S.mode === 'cover' ? 'lyrics' : 'cover');
});

document.addEventListener('mousemove', () => {
  stage.classList.remove('idle');
  document.body.style.cursor = '';
  clearTimeout(S.idleT);
  S.idleT = setTimeout(() => {
    stage.classList.add('idle');
    if (S.mode === 'lyrics') document.body.style.cursor = 'none';
  }, 2500);
});

function control(act) {
  if (act === 'toggle') {
    const p = pos();
    S.play = { ...S.play, playing: !S.play.playing, positionMs: p };
    S.playAt = performance.now();
    S.toggleUntil = performance.now() + 1000;
    stage.classList.toggle('playing', S.play.playing);
  }
  invoke('media_control', { action: act });
}
$$('.transport button').forEach((b) => b.addEventListener('click', () => control(b.dataset.act)));

$('.bar').addEventListener('click', (e) => {
  if (!S.play.hasTimeline || !S.play.canSeek) return;
  const r = e.currentTarget.getBoundingClientRect();
  const ms = clamp((e.clientX - r.left) / r.width, 0, 1) * S.play.durationMs;
  invoke('media_seek', { ms });
  S.play = { ...S.play, positionMs: ms };
  S.playAt = performance.now();
  S.seekUntil = performance.now() + 1500;
  S.li = S.wi = -2;
});

$('#style-btn').addEventListener('click', () => {
  const next = STYLES[(STYLES.indexOf(S.style) + 1) % STYLES.length];
  setStyle(next);
  invoke('set_lyric_style', { style: next }).catch(() => {});
});
$('#thumb').addEventListener('click', () => setMode('cover'));

// ------------------------------------------------------------ clock

function updateClock() {
  const d = new Date();
  $('#clock .date').textContent = d.toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short' });
  // the lock screen matches the system clock (12 / 24 hours, no AM / PM)
  $('#clock .time').textContent = LOCK
    ? d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' }).replace(/\s*[ap]\.?\s*m\.?$/i, '')
    : d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit', hour12: false });
}
setInterval(() => { if (S.open) updateClock(); }, 1000);

// ------------------------------------------------------------ loop

const miniBars = $$('.mini-bars i');
let lastT = performance.now();
let smooth = 0;
let lastSec = -1;

function frame(t) {
  if (!S.open) { S.raf = 0; return; }
  // as a wallpaper ~30 fps is plenty and easier on the battery
  if (WALL && t - lastT < 30) { S.raf = requestAnimationFrame(frame); return; }
  const dt = Math.min(100, t - lastT);
  lastT = t;
  const playing = !!S.track && S.play.playing;
  if (playing && !S.play.hasTimeline) S.localPos += dt;

  if (t - S.lastLevelAt > 120) S.level *= 0.85;
  let target = playing ? Math.min(1, Math.sqrt(S.level) * 1.1) : 0;
  if (playing && !S.gotLevel) target = 0.45 + 0.2 * Math.sin(t / 190);
  smooth += (target - smooth) * (target > smooth ? 0.5 : 0.15);
  stage.style.setProperty('--level', smooth.toFixed(3));

  // beat detection → occasional glitch stripes (like the reel)
  S.slowLevel += (smooth - S.slowLevel) * 0.04;
  if (playing && smooth - S.slowLevel > 0.16 && t - (S.lastGlitch || 0) > 900 && Math.random() < (S.style === 'words' ? 0.35 : 0.14)) {
    S.glitch = 1;
    S.lastGlitch = t;
    if ((S.style === 'words' || S.style === 'lens' || S.style === 'aura') && S.mode === 'lyrics' && !(DEMO && DEMO.get('calm'))) Words.flash();
  }
  S.glitch *= 0.8;
  if (DEMO && DEMO.get('calm')) S.glitch = 0;

  // screen-edge light: travelling sections of a hot white line (see edge.js)
  const lyr = S.mode === 'lyrics';
  // "Liquid" background: the cover keeps flowing (keeps drawing a moment
  // after leaving the lyrics so the fade is smooth)
  if (window.FlowBG && S.settings.lyricBg === 'liquid' && (lyr || t < S.glUntil)) {
    if (lyr) S.glUntil = t + 1200;
    FlowBG.draw({ time: t / 1000 + (DEMO ? Number(DEMO.get('lt') || 0) : 0), level: smooth, mode: 2, pix: 0, glitch: 0, seed: (S.track ? hashStr(S.track.key) % 97 : 0) / 10 });
  }
  if (window.Glitter) Glitter.draw(t, smooth, playing);
  if (window.EdgeLight && S.settings.glow) EdgeLight.draw(t, smooth, playing, S.lightRGB || [236, 226, 232], lyr ? 1 : 0.7);

  const p = pos();

  if (S.mode === 'cover') {
    miniBars.forEach((b, i) => {
      const k = Math.abs(Math.sin(t * 0.011 * (1 + i * 0.27) + i * 1.7));
      b.style.transform = `scaleY(${playing ? Math.max(0.25, Math.min(1, smooth * (0.4 + 0.9 * k))).toFixed(3) : 0.25})`;
    });
    const dur = S.play.durationMs;
    const pct = S.play.hasTimeline && dur > 0 ? (p / dur) * 100 : 0;
    $('.bar .fill').style.width = pct + '%';
    const sec = Math.floor(p / 1000);
    if (sec !== lastSec) {
      lastSec = sec;
      $('.row-progress .el').textContent = fmt(p);
      $('.row-progress .rem').textContent = S.play.hasTimeline ? '-' + fmt(dur - p) : '--:--';
    }
  }

  if (S.mode === 'lyrics' && S.lines.length) {
    const lp = p + 120;
    const li = lineIndex(lp);
    let wi = -1;
    if (li >= 0) {
      const ws = S.lines[li].words;
      for (let k = 0; k < ws.length; k++) if (ws[k].t <= lp) wi = k;
    }
    if (li !== S.li || wi !== S.wi) {
      const jumped = S.li === -2 || (li !== S.li && li !== S.li + 1) || (li === S.li && wi < S.wi);
      S.li = li;
      S.wi = wi;
      if (S.style === 'stars') Stars.update(li, wi, jumped);
      else if (S.style === 'terminal') Term.update(li, wi, jumped);
      else if (S.style === 'lines') Lines.update(li);
      else Words.update(li, wi, jumped);
    }
  }

  S.raf = requestAnimationFrame(frame);
}

// ------------------------------------------------------------ wiring



// liquid glass on the card and the style button
LiquidGlass.ensure('lg-card', { scale: 56, blur: 3, dispersion: 0.1 });
LiquidGlass.ensure('lg-btn', { scale: 26, blur: 1.2, dispersion: 0.12 });
function sizeGlass() {
  const cr = parseFloat(getComputedStyle(card).borderRadius) || 20;
  LiquidGlass.size('lg-card', card.offsetWidth, card.offsetHeight, cr, Math.min(card.offsetHeight * 0.32, 26));
  const b = $('#style-btn');
  LiquidGlass.size('lg-btn', b.offsetWidth, b.offsetHeight, b.offsetWidth / 2, b.offsetWidth * 0.3);
}

function applySettings(s) {
  S.settings = { ...S.settings, ...s };
  // effect, background and "Show album cover" (laid out again when any changes)
  const look = [S.settings.lyricStyle, S.settings.lyricBg, !!S.settings.lyricCover].join('|');
  if (look !== S.look) { S.look = look; setStyle(S.settings.lyricStyle || S.style); }
  if (window.Glitter) Glitter.setMode(S.settings.glitter || 'off');
  document.body.classList.toggle('no-glow', !S.settings.glow);
}
invoke('get_settings').then(applySettings).catch(() => {});
listen('settings', (e) => applySettings(e.payload));

window.addEventListener('resize', () => {
  sizeGlass();
  if (window.FlowBG) FlowBG.resize();
  if (window.EdgeLight) EdgeLight.resize();
  if (window.Glitter) Glitter.resize();
  sizeCanvases();
  Stars.reset();
  Words.reset();
  Term.reset();
  Lines.reset();
  S.li = S.wi = -2;
});

listen('full:open', (e) => open(e && e.payload));
listen('media:track', (e) => setTrack(e.payload));
listen('media:state', (e) => onState(e.payload));
listen('audio:level', (e) => {
  if (!S.open) return;
  S.level = Math.max(S.level * 0.6, e.payload);
  S.lastLevelAt = performance.now();
  S.gotLevel = true;
});

setStyle(S.style);
setArt(PLACEHOLDER, false);
updateClock();

// ------------------------------------------------------------ desktop wallpaper / lock screen

// Desktop: fades in while music plays – the lyrics while the song runs, the
// cover while it is paused or has no synced lyrics – and your own wallpaper
// comes back half a minute after the music stops.
// Lock screen: the record turns under the clock over your lock screen; click
// it for the cover, click the cover for the lyrics (in the chosen effect),
// click the lyrics to go back to the record. The player card works too.
if (WALL) {
  document.body.classList.add('wall');
  if (LOCK) document.body.classList.add('lock');
  const W = { active: false, lastPlay: -1e9, want: 'cover', wantAt: 0, hiding: false };
  const L = { state: 'record', tries: 0 };
  listen('media:state', (e) => { W.active = !!(e.payload && e.payload.active); });
  listen('media:track', (e) => { if (!e.payload || !e.payload.key) W.active = false; });

  // setMode ignores calls while an animation runs: keep asking until it's done
  const ensureMode = (m, n = 0) => {
    if (S.mode === m || n > 20) return;
    if (!S.busy) setMode(m);
    setTimeout(() => ensureMode(m, n + 1), 250);
  };
  const lockState = (st) => {
    L.state = st;
    for (const k of ['record', 'cover', 'lyrics']) stage.classList.toggle(`lk-${k}`, k === st);
    ensureMode(st === 'lyrics' ? 'lyrics' : 'cover');
    setTimeout(reportHits, 60);
    setTimeout(reportHits, 1100);
  };
  // the lock-screen window only takes the mouse over what can be clicked
  const reportHits = () => {
    if (!LOCK) return;
    const rects = [];
    if (S.open && !W.hiding) {
      const add = (el) => {
        const b = el.getBoundingClientRect();
        if (b.width > 0 && b.height > 0) rects.push({ x: b.left, y: b.top, w: b.width, h: b.height });
      };
      if (L.state !== 'lyrics' || S.style === 'lines') add(card);
      if (L.state === 'record') add($('#lock-record'));
      if (L.state === 'cover') add(artBox);
      if (L.state === 'lyrics') rects.push({ x: 0, y: innerHeight * 0.26, w: innerWidth, h: innerHeight * 0.5 });
    }
    invoke('set_lock_hit', { rects }).catch(() => {});
  };
  if (LOCK) {
    stage.classList.add('lk-record');
    // typing the password / pointing at it: uncover the login area for a moment
    listen('lock:reveal', (e) => document.body.classList.toggle('reveal', !!e.payload));
    setInterval(reportHits, 1000);
    document.addEventListener('click', (e) => {
      if (!S.open || W.hiding || e.target.closest('.card')) return;
      if (L.state === 'record') { if (e.target.closest('#lock-record')) lockState('cover'); }
      else if (L.state === 'cover') { if (e.target.closest('.art-box')) lockState(S.lines.length ? 'lyrics' : 'record'); }
      else lockState('record');
    });
  }

  const show = () => {
    S.open = true;
    W.want = 'cover';
    if (LOCK) lockState('record');
    if (window.FlowBG) FlowBG.resize();
    if (window.EdgeLight) EdgeLight.resize();
    if (window.Glitter) Glitter.resize();
    enter(true);
    requestAnimationFrame(sizeGlass);
    if (!S.raf) S.raf = requestAnimationFrame(frame);
    setTimeout(reportHits, 1900);
  };
  const hide = async () => {
    W.hiding = true;
    reportHits();
    const a = stage.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 1600, easing: 'ease-in-out', fill: 'forwards' });
    await a.finished.catch(() => {});
    S.open = false; // stops drawing
    stage.getAnimations().forEach((x) => x.cancel());
    W.hiding = false;
    reportHits();
  };
  setInterval(() => {
    const now = performance.now();
    const playing = W.active && !!S.track && S.play.playing;
    if (playing) W.lastPlay = now;
    // the lock screen shows the player whenever a song is loaded (playing or
    // paused, to resume from it); the desktop only while music plays
    const keep = W.active && !!S.track && (LOCK || playing || now - W.lastPlay < 30000);
    if (LOCK) {
      const why = `active=${W.active} track=${S.track ? S.track.title : '-'} playing=${playing} keep=${keep} open=${S.open}`;
      if (why !== W.why) { W.why = why; invoke('debug_log', { msg: why }).catch(() => {}); }
    }
    if (keep && !S.open) show();
    else if (!keep && S.open && !W.hiding) hide();
    if (!S.open || W.hiding || LOCK) return;
    const want = playing && S.lines.length ? 'lyrics' : 'cover';
    if (want !== W.want) { W.want = want; W.wantAt = now; }
    if (want !== S.mode && !S.busy && now - W.wantAt > 1200) setMode(want);
  }, 400);
  if (DEMO && DEMO.get('lk')) setTimeout(() => lockState(DEMO.get('lk')), 1500);
  if (DEMO && DEMO.get('reveal')) document.body.classList.add('reveal');
  invoke('media_resend').catch(() => {});
}
