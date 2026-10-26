// Showcase cards, the scroll-driven stage, lazy clip playback and copy buttons. No dependencies.
(function () {
  "use strict";
  // Descriptions are the @description lines of shaders/<id>.glsl.
  var EFFECTS = [
    { id: "glitch", name: "Glitch", tag: "Wallpaper effect", c: "#ff00a8",
      desc: "Your wallpaper, untouched until a kick lands: a brief RGB split and a few slipped slices, then calm" },
    { id: "shards", name: "Shards", tag: "Over your wallpaper", c: "#00e5ff",
      desc: "Your wallpaper seen through faceted glass: each shard bends it a little, kicks crack it open, highs glint on the bevels" },
    { id: "nebula", name: "Nebula", tag: "Generative", c: "#ff00a8",
      desc: "Slow cosmic clouds folding into themselves; they bloom on every kick while stars catch the highs" },
    { id: "synthwave", name: "Synthwave", tag: "Generative", c: "#ff7a3d",
      desc: "Retro sunset over a neon grid, with a mountain range drawn by the spectrum" },
    { id: "woofer", name: "Woofer", tag: "Wallpaper effect", c: "#00e5ff",
      desc: "Your wallpaper as a speaker cone: the bass pushes it out, every kick makes it ring" },
    { id: "ripple", name: "Ripple", tag: "Wallpaper effect", c: "#ff7a3d",
      desc: "Each kick drops a stone in your wallpaper: one clean ring travels out from the centre and fades" },
    { id: "aurora", name: "Aurora", tag: "Over your wallpaper", c: "#ff00a8",
      desc: "Northern-light curtains drifting over your wallpaper, rippling and brightening with the mids" }
  ];
  var reduce = matchMedia("(prefers-reduced-motion: reduce)");
  var $ = function (id) { return document.getElementById(id); };

  // ---- cards
  var track = $("track");
  EFFECTS.forEach(function (e, i) {
    var fig = document.createElement("figure");
    fig.className = "card";
    fig.style.setProperty("--c", e.c);
    fig.innerHTML =
      '<div class="card-media" style="background-image:url(clips/' + e.id + '.jpg)">' +
        '<video muted loop playsinline preload="none" poster="clips/' + e.id + '.jpg" aria-label="' + e.name + ' effect, looping clip">' +
          '<source src="clips/' + e.id + '.mp4" type="video/mp4"></video></div>' +
      '<figcaption><span class="num">' + String(i + 1).padStart(2, "0") + '</span>' +
        '<h3>' + e.name + '<span class="tag">' + e.tag + '</span></h3><p>' + e.desc + '.</p></figcaption>';
    track.appendChild(fig);
  });
  $("tot").textContent = String(EFFECTS.length).padStart(2, "0");
  var cards = [].slice.call(track.children);
  var videos = cards.map(function (c) { return c.querySelector("video"); });

  // ---- playback: only clips on screen play; reduced motion gets controls instead of autoplay
  function applyMotion() {
    var hero = document.querySelector(".hero-bg");
    if (reduce.matches) {
      hero.removeAttribute("autoplay"); hero.pause();
      videos.forEach(function (v) { v.controls = true; v.pause(); });
    } else {
      hero.play().catch(function () {});
      videos.forEach(function (v) { v.controls = false; });
    }
  }
  var visible = new Set();
  var io = new IntersectionObserver(function (entries) {
    entries.forEach(function (en) {
      var v = en.target;
      if (en.isIntersecting) { visible.add(v); if (!reduce.matches) { v.preload = "auto"; v.play().catch(function () {}); } }
      else { visible.delete(v); v.pause(); }
    });
  }, { rootMargin: "0px 20% 0px 20%", threshold: 0.15 });
  videos.forEach(function (v) { io.observe(v); });

  // ---- sticky stage: vertical scroll drives the horizontal track
  var section = $("effects"), stage = section.querySelector(".stage"), bar = $("bar"), cur = $("cur");
  // stops[i]: track offset that centres card i. Between two stops the track eases, so each card holds centre stage.
  var stops = [], lastIdx = -1;
  function layout() {
    if (reduce.matches) { section.style.height = ""; track.style.transform = ""; return; }
    stops = cards.map(function (c) { return c.offsetLeft + c.offsetWidth / 2 - innerWidth / 2; });
    section.style.height = ((cards.length - 1) * innerHeight * 0.8 + stage.offsetHeight * 1.2) + "px";
    update();
  }
  function smooth(t) { t = Math.min(1, Math.max(0, (t - 0.18) / 0.64)); return t * t * (3 - 2 * t); }
  function update() {
    if (reduce.matches) return;
    var r = section.getBoundingClientRect();
    var span = section.offsetHeight - stage.offsetHeight;
    var p = span > 0 ? Math.min(1, Math.max(0, -r.top / span)) : 0;
    var f = p * (cards.length - 1), k = Math.min(cards.length - 2, Math.floor(f));
    var x = stops[k] + (stops[k + 1] - stops[k]) * smooth(f - k);
    track.style.transform = "translate3d(" + (-x).toFixed(1) + "px,0,0)";
    bar.style.transform = "scaleX(" + p.toFixed(4) + ")";
    var mid = innerWidth / 2, best = 0, bestD = Infinity;
    cards.forEach(function (c, i) {
      var b = c.getBoundingClientRect();
      var d = (b.left + b.width / 2 - mid) / innerWidth;
      var a = Math.min(1, Math.abs(d) * 1.6);
      var m = c.firstChild;
      m.style.transform = "perspective(1400px) rotateY(" + (-d * 14).toFixed(2) + "deg) scale(" + (1 - a * 0.1).toFixed(3) + ")";
      c.style.opacity = (1 - a * 0.55).toFixed(3);
      if (Math.abs(d) < bestD) { bestD = Math.abs(d); best = i; }
    });
    if (best !== lastIdx) {
      lastIdx = best;
      cur.textContent = String(best + 1).padStart(2, "0");
      stage.style.setProperty("--accent", EFFECTS[best].c);
    }
  }
  var ticking = false;
  addEventListener("scroll", function () {
    if (ticking) return; ticking = true;
    requestAnimationFrame(function () { ticking = false; update(); });
  }, { passive: true });
  addEventListener("resize", layout);
  new ResizeObserver(layout).observe(track);
  reduce.addEventListener("change", function () {
    cards.forEach(function (c) { c.style.opacity = ""; c.firstChild.style.transform = ""; });
    applyMotion(); layout();
  });
  applyMotion();
  layout();
  if (document.fonts) document.fonts.ready.then(layout);

  // ---- copy buttons
  document.querySelectorAll("[data-copy]").forEach(function (box) {
    var btn = box.querySelector(".copy");
    btn.onclick = function () {
      navigator.clipboard.writeText(box.querySelector("code").textContent.trim()).then(function () {
        btn.textContent = "Copied"; btn.classList.add("done");
        setTimeout(function () { btn.textContent = "Copy"; btn.classList.remove("done"); }, 1600);
      });
    };
  });
})();
