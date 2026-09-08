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

## Deployment

The existing Cloudflare configuration is preserved in `wrangler.jsonc`.
Moving the source files does not change the existing deployment connection.
Automatic deployment from this repository still requires updating that
connection to use `chriopter/omalux` and the `omalux.org/` subdirectory.
