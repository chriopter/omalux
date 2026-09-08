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

## Screenshot

From any directory, run `omalux.org/scripts/screenshot.sh` using the appropriate
path to this checkout. It builds the current GUI and captures the real app at
1440×920 with the repository beach image, replacing `public/app-screenshot.png`.
It requires the GUI build dependencies, Python 3, and GNU coreutils. No desktop
session is needed; the capture uses the current app theme and installed fonts.

```bash
# From the repository root:
omalux.org/scripts/screenshot.sh
omalux.org/scripts/screenshot.sh /path/to/photo.jpg /tmp/omalux-screenshot.png
```

The previous screenshot survives a build or capture failure. Review the resulting
image before committing it. If you choose a different subject, update the image's
alt text in `src/pages/index.astro` as needed.

## Deployment

The existing Cloudflare configuration is preserved in `wrangler.jsonc`.
Moving the source files does not change the existing deployment connection.
Automatic deployment from this repository still requires updating that
connection to use `chriopter/omalux` and the `omalux.org/` subdirectory.
