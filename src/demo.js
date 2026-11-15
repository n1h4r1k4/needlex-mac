// The made-up song the pages play when opened in a plain browser (no app):
// handy for working on the UI and for the screenshots in docs/. The title,
// lyrics and cover are original placeholders.
const DemoSong = {
  key: 'demo',
  title: 'City Lights',
  artist: 'NeedleX Demo',
  album: 'Demo Tapes',
  lines: [
    'City lights are calling out my name',
    'Dancing with the shadows in the rain',
    'Turn it up and let the night begin',
    '',
    'Hold me close and never let it end',
    'Every song is shining like the stars',
  ],
  /** synced lyrics, one line every `step` ms from `from` */
  synced(from = 0, step = 4000) {
    return this.lines.map((text, i) => ({ time: from + i * step, text }));
  },
  /** the cover: a retro sunset (data URL) */
  art(size = 600) {
    const c = document.createElement('canvas');
    c.width = c.height = size;
    const g = c.getContext('2d');
    const s = size / 300;
    g.scale(s, s);
    const sky = g.createLinearGradient(0, 0, 0, 300);
    sky.addColorStop(0, '#2a1046');
    sky.addColorStop(0.45, '#c2185b');
    sky.addColorStop(0.75, '#ff7a59');
    sky.addColorStop(1, '#ffc46b');
    g.fillStyle = sky;
    g.fillRect(0, 0, 300, 300);
    // stars
    g.fillStyle = 'rgba(255,255,255,.8)';
    for (let i = 0; i < 40; i++) {
      const x = (i * 73) % 300, y = (i * 37) % 110;
      g.fillRect(x, y, 1.4, 1.4);
    }
    // striped sun
    const sun = g.createLinearGradient(0, 80, 0, 220);
    sun.addColorStop(0, '#ffe08a');
    sun.addColorStop(1, '#ff5e7e');
    g.save();
    g.beginPath();
    g.arc(150, 160, 78, 0, Math.PI * 2);
    g.clip();
    g.fillStyle = sun;
    g.fillRect(60, 80, 180, 160);
    g.fillStyle = sky;
    for (let k = 0; k < 6; k++) g.fillRect(60, 168 + k * 13, 180, 2 + k * 1.3);
    g.restore();
    // hills
    g.fillStyle = '#3b1250';
    g.beginPath();
    g.moveTo(0, 300); g.lineTo(0, 215);
    g.bezierCurveTo(60, 180, 110, 230, 160, 210);
    g.bezierCurveTo(210, 190, 250, 200, 300, 220);
    g.lineTo(300, 300); g.fill();
    g.fillStyle = '#1a0a2a';
    g.beginPath();
    g.moveTo(0, 300); g.lineTo(0, 250);
    g.bezierCurveTo(80, 230, 140, 270, 200, 252);
    g.bezierCurveTo(240, 240, 270, 245, 300, 255);
    g.lineTo(300, 300); g.fill();
    return c.toDataURL('image/jpeg', 0.92);
  },
};

// The screenshot script can swap in another song (scripts/screenshots.sh).
if (window.DemoSongOverride) Object.assign(DemoSong, window.DemoSongOverride);
