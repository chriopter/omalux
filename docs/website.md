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

The website and repository README share real screenshots of the current darktable-based development UI. The editing view is `public/app-screenshot-dark.png`; the preset view is `public/app-presets-dark.png`. Both website themes use these same images, matching the app's current dark appearance. Keep the development-preview label visible.

Generate both from the repository root:

```bash
omalux.org/scripts/screenshots.sh
# Or capture one view:
omalux.org/scripts/screenshot.sh assets/images/beach-volleyball.jpg /tmp/omalux-presets.png presets
```

The scripts build the current native app if needed and capture its real QML window with the shared beach photograph at 1280×820 logical pixels and 2× display scale (2560×1640 PNG). They use separate temporary app settings and darktable databases. The running editing session stays untouched. GTK/OpenCL still require a working desktop session, although the Qt window is captured offscreen. Native build dependencies and Python 3 are required. Existing outputs survive failed captures.

Review both images before committing. The gallery alternates editing and preset views; page light/dark switching changes the website colors, not the captured application's theme.

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
