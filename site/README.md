# Woofer website

Static site for GitHub Pages. No build step: everything the page needs is in this folder.

```
index.html  style.css  main.js     page, styles, showcase stage
live.js                            live WebGL preview (trimmed from web/app.js)
live/                              copies of web/shaders.generated.js and quickshell/analysis.js
clips/<id>.mp4 + .jpg              one 3.9 s loop per featured effect, with its poster
img/                               logo and the wallpaper used by the clips and the live preview
sync.sh                            refreshes live/ after a preset changes
```

Fonts come from jsdelivr (`@fontsource-variable/*@5.2.8`, pinned). No trackers, no cookies.

## Preview locally

```bash
python -m http.server 4420 --bind 127.0.0.1    # from the repo root, then open http://127.0.0.1:4420/site/
```

## After changing a preset

```bash
node tools/build.mjs && site/sync.sh
```

## Re-recording a clip

Clips are recorded with the browser preview (`python -I tools/devserver.py 4421`, then
`/web/?mute=1&rec=wide&save=put&wall=local/gif/cosmic-cliffs.jpg&colors=00e5ff,ff00a8,ff7a3d&seq=<id>:5:<intensity>&preset=<id>`,
click Record with the page visible). The recording keeps alpha, so presets that draw over the wallpaper are composited
back onto it, then cut to 8 beats at 124 bpm with a 0.4 s crossfade so the loop is seamless:

```bash
src=web/local/gif/woofer-<id>-<timestamp>.webm
ffmpeg -ss 0.5 -c:v libvpx-vp9 -i "$src" -loop 1 -i web/local/gif/cosmic-cliffs.jpg -filter_complex \
 "[1]scale=1280:720:force_original_aspect_ratio=increase,crop=1280:720[w];[w][0]overlay=shortest=1,fps=30,scale=960:540:flags=lanczos,trim=0:4.271,setpts=PTS-STARTPTS,split[a][b];
  [a]trim=0.4:4.271,setpts=PTS-STARTPTS[main];[b]trim=0:0.4,setpts=PTS-STARTPTS[head];
  [main][head]xfade=transition=fade:duration=0.4:offset=3.471,format=yuv420p" \
 -an -c:v libx264 -preset slow -crf 23 -movflags +faststart site/clips/<id>.mp4
ffmpeg -ss 0.2 -i site/clips/<id>.mp4 -frames:v 1 -q:v 6 site/clips/<id>.jpg
```

Intensities used: glitch 3.2, shards 1.6, nebula 1.3, synthwave 1.2, woofer 2.6, ripple 2.4, aurora 1.0.
Beat-driven effects last ~0.15 s, so they need more intensity than the daemon default to read in a short loop.

## Publishing on GitHub Pages

Pages can only serve the repo root or `docs/` from a branch, so `site/` is deployed with an Actions workflow.
Add `.github/workflows/pages.yml`:

```yaml
name: Pages
on:
  push: { branches: [main], paths: ["site/**"] }
  workflow_dispatch:
permissions: { contents: read, pages: write, id-token: write }
concurrency: { group: pages, cancel-in-progress: true }
jobs:
  deploy:
    runs-on: ubuntu-latest
    environment: { name: github-pages, url: "${{ steps.deploy.outputs.page_url }}" }
    steps:
      - uses: actions/checkout@v4
      - uses: actions/upload-pages-artifact@v3
        with: { path: site }
      - id: deploy
        uses: actions/deploy-pages@v4
```

Then in the repository settings: Pages → Source → GitHub Actions. The site lands at https://ybucaille.github.io/woofer/.

## Credits

Wallpaper: “Cosmic Cliffs” in the Carina Nebula, James Webb Space Telescope.
Credit: NASA, ESA, CSA, and STScI, CC BY 4.0 (https://esawebb.org/images/weic2205b/).
