# omalux.org

The Omalux website, built with Astro and the Cloudflare adapter.
Imported from https://github.com/chriopter/omalux.org at commit `560c772`.

## Development

Requires Node.js 22.12.0 or newer. From this repository’s root:

```bash
cd omalux.org
npm ci
npm run dev
```

Edit `src/pages/index.astro` for the landing page and `public/` for images
and other static assets. Run `npm run build` to check the production build.

The Lux wordmark is `public/assets/omalux-logo.svg`; `public/favicon.svg` and
`public/favicon.ico` use its pixel sun. The header adapts the logo to the selected
light/dark theme and uses a sun/moon button to switch modes.

## Screenshot

From any directory, run `omalux.org/scripts/screenshots.sh` using the appropriate
path to this checkout. It builds the current GUI and captures the real app at
1440×920 with the repository beach image, replacing `public/app-screenshot.png`
(light) and `public/app-screenshot-dark.png` (dark).
It requires the GUI build dependencies, Python 3, and GNU coreutils. No desktop
session is needed. The pair uses fixed light/dark palettes and installed fonts;
the desktop theme is not changed. The website follows the system color scheme
initially and remembers the visitor's selection using local storage. The header
switch changes both the page colors and the displayed app screenshot.

```bash
# From the repository root:
omalux.org/scripts/screenshots.sh
omalux.org/scripts/screenshots.sh /path/to/photo.jpg
# Capture one image; omit the last argument to use the current app theme:
omalux.org/scripts/screenshot.sh /path/to/photo.jpg /tmp/omalux-screenshot.png dark
```

The previous screenshot survives a build or capture failure. Review the resulting
image before committing it. If you choose a different subject, update the image's
alt text in `src/pages/index.astro` as needed.

## Deployment

Cloudflare Workers Builds is connected to `chriopter/omalux`, branch `main`,
with root directory `/omalux.org`, build command `npm run build`, and deploy
command `npx wrangler deploy`. Pushing website changes publishes the static site.

The root directory does not restrict which changes trigger a build. In Cloudflare
Settings → Builds → Build watch paths, replace the include rule `*` with
`omalux.org/*` to build only for changes under the website directory (including
its nested folders). This dashboard setting is not configured by `wrangler.jsonc`.
See https://developers.cloudflare.com/workers/ci-cd/builds/build-watch-paths/.
Regenerate and commit screenshots explicitly after GUI changes when needed.
