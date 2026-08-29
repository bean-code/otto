# otto-ui frontend

Plain HTML/JS (see the root README for why), styled with [Tailwind CSS](https://tailwindcss.com) v4 - but
**compiled once, ahead of time**, not loaded from a CDN or built on every `cargo build`. `dist/assets/tailwind.css`
is a checked-in, static asset; the app never needs Node/npm to run, only to *edit the styling*.

## Rebuilding the CSS

Only needed if you change `src/input.css` (the `@theme` tokens, custom `@utility`/`@keyframes`) or add Tailwind
classes to `dist/index.html` that aren't in the current build:

```sh
cd crates/otto-ui/frontend/src
npm install --no-save tailwindcss @tailwindcss/cli
./node_modules/.bin/tailwindcss -i input.css -o ../dist/assets/tailwind.css --minify
rm -rf node_modules   # optional - it's gitignored either way
```

`input.css`'s `@source "../dist/index.html"` tells Tailwind where to scan for class names; it picks up classes
even inside JS template literals (plain substring matching), so dynamically-built class strings in `index.html`'s
`<script>` still compile correctly as long as each individual class name appears somewhere in the file as a plain
string.

## Icons

No emoji, no icon font, no external icon package - a small hand-authored set of inline SVGs (outline style, 24x24
viewBox, `currentColor` stroke) defined as raw path data in the `ICONS` map at the top of `index.html`'s script,
rendered via the local `iconSvg(name, classes)` helper. Add a new one by adding an entry to that map.
