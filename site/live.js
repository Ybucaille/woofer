// Live preview: the real presets (live/shaders.generated.js, copied from web/ by site/sync.sh) and the shared
// analysis (live/analysis.js), trimmed from web/app.js. Nothing runs until the visitor presses play,
// and it stops drawing while the player is off screen.
(function () {
  "use strict";
  var BANDS = 16;
  var $ = function (id) { return document.getElementById(id); };
  var canvas = $("live-canvas"), startBtn = $("live-start");
  var presets = window.RD_PRESETS || [];
  var gl = canvas.getContext("webgl2", { premultipliedAlpha: true, alpha: true });
  if (!gl || !presets.length || typeof rdCreateState !== "function") {
    startBtn.hidden = true;
    $("live-desc").textContent = "Your browser has no WebGL 2, so the live preview can't run here. The clips above show every effect.";
    return;
  }

  var VERT = "#version 300 es\nin vec2 p;\nvoid main(){ gl_Position = vec4(p, 0.0, 1.0); }";
  var quad = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, quad);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  function compile(type, src) {
    var s = gl.createShader(type);
    gl.shaderSource(s, src); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    return s;
  }
  var programs = {};
  function program(preset) {
    if (programs[preset.id]) return programs[preset.id];
    var p = gl.createProgram();
    gl.attachShader(p, compile(gl.VERTEX_SHADER, VERT));
    gl.attachShader(p, compile(gl.FRAGMENT_SHADER, preset.source));
    gl.bindAttribLocation(p, 0, "p");
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
    var u = {};
    ["iTime", "iResolution", "uFlow", "uLevel", "uBass", "uMid", "uHigh", "uBeat", "uSpec0", "uSpec1", "uSpec2", "uSpec3",
     "uColor1", "uColor2", "uColor3", "uIntensity", "iWallpaper", "iWallpaperSize"].forEach(function (n) { u[n] = gl.getUniformLocation(p, n); });
    return (programs[preset.id] = { prog: p, u: u });
  }

  // ---- wallpaper
  var wallTex = gl.createTexture(), wallSize = [1, 1];
  gl.bindTexture(gl.TEXTURE_2D, wallTex);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([10, 8, 20, 255]));
  function loadWallpaper(url) {
    var img = new Image();
    img.onload = function () {
      gl.bindTexture(gl.TEXTURE_2D, wallTex);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
      gl.generateMipmap(gl.TEXTURE_2D);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.MIRRORED_REPEAT);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.MIRRORED_REPEAT);
      wallSize = [img.naturalWidth, img.naturalHeight];
      canvas.parentNode.style.backgroundImage = "url(" + url + ")";
    };
    img.src = url;
  }
  $("live-wall").onchange = function (e) { var f = e.target.files[0]; if (f) loadWallpaper(URL.createObjectURL(f)); };

  // ---- preset chips
  var current = presets.find(function (p) { return p.id === "glitch"; }) || presets[0];
  var chips = $("live-presets");
  presets.forEach(function (p) {
    var b = document.createElement("button");
    b.type = "button"; b.className = "chip"; b.textContent = p.name; b.dataset.id = p.id;
    b.setAttribute("role", "radio");
    b.onclick = function () { select(p); };
    chips.appendChild(b);
  });
  function select(p) {
    current = p;
    [].forEach.call(chips.children, function (b) { b.setAttribute("aria-checked", String(b.dataset.id === p.id)); });
    $("live-desc").textContent = p.description + ".";
  }
  select(current);
  var intensity = 1;
  $("live-int").oninput = function (e) { intensity = parseFloat(e.target.value); };

  // ---- audio
  var ac = null, analyser = null, freq = null, source = null, out = null, sound = true;
  function ensureAudio() {
    if (ac) return;
    ac = new AudioContext();
    analyser = ac.createAnalyser();
    analyser.fftSize = 4096; analyser.smoothingTimeConstant = 0.5;
    freq = new Uint8Array(analyser.frequencyBinCount);
    out = ac.createGain(); out.gain.value = sound ? 1 : 0; out.connect(ac.destination);
  }
  function connect(node, audible) {
    ensureAudio();
    if (source) source.disconnect();
    source = node; node.connect(analyser);
    if (audible) node.connect(out);
    ac.resume();
  }
  var beatOn = false;
  // Synthetic 124 bpm kick + bass + hats, the same as the browser preview's demo beat.
  function demoBeat() {
    ensureAudio();
    var bus = ac.createGain(); bus.gain.value = 0.3;
    var beat = 60 / 124, t0 = ac.currentTime + 0.05;
    var noise = ac.createBuffer(1, ac.sampleRate * 0.2, ac.sampleRate), nd = noise.getChannelData(0);
    for (var i = 0; i < nd.length; i++) nd[i] = Math.random() * 2 - 1;
    var notes = [55, 55, 65.4, 49];
    function schedule(n) {
      var t = t0 + n * beat;
      var k = ac.createOscillator(), kg = ac.createGain();
      k.frequency.setValueAtTime(150, t); k.frequency.exponentialRampToValueAtTime(40, t + 0.15);
      kg.gain.setValueAtTime(1, t); kg.gain.exponentialRampToValueAtTime(0.001, t + 0.35);
      k.connect(kg).connect(bus); k.start(t); k.stop(t + 0.4);
      var b = ac.createOscillator(), bg = ac.createGain();
      b.type = "sawtooth"; b.frequency.value = notes[Math.floor(n / 4) % 4];
      bg.gain.setValueAtTime(0, t + beat / 2); bg.gain.linearRampToValueAtTime(0.25, t + beat / 2 + 0.02); bg.gain.exponentialRampToValueAtTime(0.001, t + beat);
      b.connect(bg).connect(bus); b.start(t + beat / 2); b.stop(t + beat);
      var h = ac.createBufferSource(), hf = ac.createBiquadFilter(), hg = ac.createGain();
      h.buffer = noise; hf.type = "highpass"; hf.frequency.value = 7000;
      hg.gain.setValueAtTime(n % 2 ? 0.35 : 0.15, t + beat / 2); hg.gain.exponentialRampToValueAtTime(0.001, t + beat / 2 + 0.06);
      h.connect(hf).connect(hg).connect(bus); h.start(t + beat / 2);
    }
    connect(bus, true);
    var n = 0;
    (function loop() {
      if (source !== bus) return; // replaced by the mic
      while (t0 + n * beat < ac.currentTime + 1) schedule(n++);
      setTimeout(loop, 250);
    })();
  }
  $("live-sound").onclick = function () {
    sound = !sound;
    if (out) out.gain.value = sound ? 1 : 0;
    this.setAttribute("aria-pressed", String(sound));
    this.textContent = sound ? "Sound on" : "Sound off";
  };
  $("live-mic").onclick = function () {
    var btn = this;
    navigator.mediaDevices.getUserMedia({ audio: true }).then(function (s) {
      ensureAudio(); connect(ac.createMediaStreamSource(s), false);
      btn.textContent = "Listening to your mic"; run();
    }).catch(function () { btn.textContent = "Mic unavailable"; });
  };
  startBtn.onclick = function () { demoBeat(); run(); };

  // ---- render
  var raw = new Array(BANDS).fill(0), state = rdCreateState(BANDS);
  function readBands() {
    if (!analyser) return;
    analyser.getByteFrequencyData(freq);
    var nyq = ac.sampleRate / 2, lo = 40, hi = 14000;
    for (var b = 0; b < BANDS; b++) {
      var f0 = lo * Math.pow(hi / lo, b / BANDS), f1 = lo * Math.pow(hi / lo, (b + 1) / BANDS);
      var i0 = Math.floor(f0 / nyq * freq.length), i1 = Math.max(i0 + 1, Math.floor(f1 / nyq * freq.length));
      var m = 0;
      for (var i = i0; i < i1; i++) m = Math.max(m, freq[i]);
      raw[b] = Math.pow(m / 255, 2);
    }
  }
  var COLORS = [[0, 0.898, 1, 1], [1, 0, 0.659, 1], [1, 0.478, 0.239, 1]]; // 00e5ff, ff00a8, ff7a3d
  var running = false, onScreen = true, start = performance.now(), last = start;
  new IntersectionObserver(function (en) {
    onScreen = en[0].isIntersecting;
    if (onScreen && running) requestAnimationFrame(frame);
  }).observe(canvas);
  function run() {
    startBtn.hidden = true;
    if (running) return;
    running = true; last = performance.now();
    requestAnimationFrame(frame);
  }
  function frame(now) {
    if (!onScreen) return;
    var dt = (now - last) / 1000; last = now;
    var dpr = Math.min(devicePixelRatio || 1, 1.5);
    var w = Math.floor(canvas.clientWidth * dpr), h = Math.floor(canvas.clientHeight * dpr);
    if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
    readBands();
    var s = rdAnalyse(state, raw, dt);
    gl.viewport(0, 0, w, h);
    gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT);
    try {
      var P = program(current), u = P.u;
      gl.useProgram(P.prog);
      gl.bindBuffer(gl.ARRAY_BUFFER, quad);
      gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
      gl.uniform1f(u.iTime, (now - start) / 1000);
      gl.uniform2f(u.iResolution, w, h);
      gl.uniform1f(u.uFlow, s.flow); gl.uniform1f(u.uLevel, s.level);
      gl.uniform1f(u.uBass, s.bass); gl.uniform1f(u.uMid, s.mid); gl.uniform1f(u.uHigh, s.high); gl.uniform1f(u.uBeat, s.beat);
      for (var q = 0; q < 4; q++) gl.uniform4f(u["uSpec" + q], s.spec[q * 4], s.spec[q * 4 + 1], s.spec[q * 4 + 2], s.spec[q * 4 + 3]);
      gl.uniform4fv(u.uColor1, COLORS[0]); gl.uniform4fv(u.uColor2, COLORS[1]); gl.uniform4fv(u.uColor3, COLORS[2]);
      gl.uniform1f(u.uIntensity, intensity);
      gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, wallTex);
      gl.uniform1i(u.iWallpaper, 0); gl.uniform2f(u.iWallpaperSize, wallSize[0], wallSize[1]);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    } catch (e) {
      console.error(current.id, e.message);
    }
    requestAnimationFrame(frame);
  }
  loadWallpaper("img/cosmic-cliffs.jpg");
})();
