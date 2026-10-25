// Browser preview: same presets and same analysis as the Quickshell layer.
// Query params: ?preset=<id>&demo=1&ui=0&intensity=1
(function () {
  "use strict";
  var BANDS = 16;
  var params = new URLSearchParams(location.search);
  var canvas = document.getElementById("c");
  var gl = canvas.getContext("webgl2", { premultipliedAlpha: true, alpha: true, preserveDrawingBuffer: true });
  var $ = function (id) { return document.getElementById(id); };

  var VERT = "#version 300 es\nin vec2 p;\nvoid main(){ gl_Position = vec4(p, 0.0, 1.0); }";
  var quad = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, quad);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);

  function compile(type, src) {
    var s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
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
     "uColor1", "uColor2", "uColor3", "uIntensity"].forEach(function (n) { u[n] = gl.getUniformLocation(p, n); });
    return (programs[preset.id] = { prog: p, u: u });
  }

  // ---- presets UI
  var presets = window.RD_PRESETS || [];
  var sel = $("preset");
  presets.forEach(function (p) { var o = document.createElement("option"); o.value = p.id; o.textContent = p.name; sel.appendChild(o); });
  var current = presets.find(function (p) { return p.id === params.get("preset"); }) || presets[0];
  sel.value = current.id;
  function showDesc() { $("desc").textContent = current.description + (current.author ? " — " + current.author : ""); }
  sel.onchange = function () { current = presets.find(function (p) { return p.id === sel.value; }); showDesc(); history.replaceState(null, "", "?preset=" + current.id); };
  showDesc();
  var intensity = parseFloat(params.get("intensity") || "1");
  $("intensity").value = intensity;
  $("intensity").oninput = function (e) { intensity = parseFloat(e.target.value); };
  function toggleUi() { $("ui").classList.toggle("hidden"); }
  $("hide").onclick = toggleUi;
  addEventListener("keydown", function (e) { if (e.key === "h" || e.key === "H") toggleUi(); });
  if (params.get("ui") === "0") toggleUi();

  // ---- audio
  var ac = null, analyser = null, freq = null, source = null;
  function ensureAudio() {
    if (ac) return;
    ac = new AudioContext();
    analyser = ac.createAnalyser();
    analyser.fftSize = 4096;
    analyser.smoothingTimeConstant = 0.5;
    freq = new Uint8Array(analyser.frequencyBinCount);
  }
  function connect(node, audible) {
    ensureAudio();
    if (source) source.disconnect();
    source = node;
    node.connect(analyser);
    if (audible) node.connect(ac.destination);
    ac.resume();
  }

  // Synthetic 124 bpm kick + bass + hats so presets can be judged without any audio file.
  function demoBeat() {
    ensureAudio();
    var bus = ac.createGain();
    bus.gain.value = 0.35;
    var bpm = 124, beat = 60 / bpm, t0 = ac.currentTime + 0.05;
    var noise = ac.createBuffer(1, ac.sampleRate * 0.2, ac.sampleRate);
    var nd = noise.getChannelData(0);
    for (var i = 0; i < nd.length; i++) nd[i] = Math.random() * 2 - 1;
    var bassNotes = [55, 55, 65.4, 49];
    function schedule(n) {
      var t = t0 + n * beat;
      var k = ac.createOscillator(), kg = ac.createGain();
      k.frequency.setValueAtTime(150, t); k.frequency.exponentialRampToValueAtTime(40, t + 0.15);
      kg.gain.setValueAtTime(1, t); kg.gain.exponentialRampToValueAtTime(0.001, t + 0.35);
      k.connect(kg).connect(bus); k.start(t); k.stop(t + 0.4);
      var b = ac.createOscillator(), bg = ac.createGain();
      b.type = "sawtooth"; b.frequency.value = bassNotes[Math.floor(n / 4) % 4];
      bg.gain.setValueAtTime(0, t + beat / 2); bg.gain.linearRampToValueAtTime(0.25, t + beat / 2 + 0.02); bg.gain.exponentialRampToValueAtTime(0.001, t + beat);
      b.connect(bg).connect(bus); b.start(t + beat / 2); b.stop(t + beat);
      var h = ac.createBufferSource(), hf = ac.createBiquadFilter(), hg = ac.createGain();
      h.buffer = noise; hf.type = "highpass"; hf.frequency.value = 7000;
      hg.gain.setValueAtTime(n % 2 ? 0.35 : 0.15, t + beat / 2); hg.gain.exponentialRampToValueAtTime(0.001, t + beat / 2 + 0.06);
      h.connect(hf).connect(hg).connect(bus); h.start(t + beat / 2);
    }
    var n = 0;
    (function loop() { while (t0 + n * beat < ac.currentTime + 1) schedule(n++); setTimeout(loop, 250); })();
    connect(bus, true);
  }
  $("demo").onclick = demoBeat;
  $("mic").onclick = function () {
    navigator.mediaDevices.getUserMedia({ audio: true }).then(function (s) { ensureAudio(); connect(ac.createMediaStreamSource(s), false); });
  };
  $("fileBtn").onclick = function () { $("file").click(); };
  $("file").onchange = function (e) {
    var f = e.target.files[0]; if (!f) return;
    var a = new Audio(URL.createObjectURL(f)); a.loop = true;
    ensureAudio(); connect(ac.createMediaElementSource(a), true); a.play();
  };
  if (params.get("demo") === "1") demoBeat();

  // 16 log-spaced bands, 40 Hz – 14 kHz, like cava's layout.
  var raw = new Array(BANDS).fill(0);
  function readBands() {
    if (!analyser) { raw.fill(0); return; }
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

  // ---- render loop
  var state = rdCreateState(BANDS);
  window.__rd = { state: state }; // for automated checks
  var COLORS = [[0.65, 0.78, 1, 1], [0.84, 0.75, 0.89, 1], [1, 0.71, 0.67, 1]];
  var start = performance.now(), last = start;
  function frame(now) {
    var dt = (now - last) / 1000; last = now;
    var w = Math.floor(canvas.clientWidth * devicePixelRatio), h = Math.floor(canvas.clientHeight * devicePixelRatio);
    if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
    readBands();
    var s = rdAnalyse(state, raw, dt);
    $("meters").textContent = "lvl " + s.level.toFixed(2) + "  bass " + s.bass.toFixed(2) + "  beat " + s.beat.toFixed(2);

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
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      window.__rd.error = null;
    } catch (e) {
      if (window.__rd.error !== e.message) console.error(current.id, e.message);
      window.__rd.error = e.message;
    }
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
})();
