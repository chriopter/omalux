# Claude review and source cross-check

2026-09-08. The user requested Claude's participation in the darktable analysis. A broad read-only Claude Code investigation was started; while it had not returned, a second, bounded review was given the exact cache, OpenCL fallback, colisa, engine and updated control-registry excerpts. The focused review returned successfully. It explicitly did not execute code or use tools. This document records the independently checked conclusions rather than treating model output as authoritative.

## Confirmed

- `DT_DEV_PIXELPIPE_IMAGE` prevents intermediate cache reuse in our current pipe. The focused review had not seen the helper implementation; we checked `src/develop/pixelpipe_hb.h:306`, which directly tests that flag. See `pixelpipe_hb.c:3157` and `pixelpipe_cache.c:178`.
- Reading `opencl_error` only after rendering misses the normal GPU-error-to-CPU-restart path, which clears it. See `pixelpipe_hb.c:3243–3284`.
- The updated colisa controls match the module's native unitless range −1 to +1, default zero and identity conversion. Separately, we verified the actual GTK formatting: `src/iop/colisa.c:267` and `src/develop/imageop_gui.c:97–110` give two decimal places with no percentage format override.
- Raw module parameter pointers need a lifecycle contract before adding history reload, presets or image/module replacement. This is a future invalidation risk, not evidence that today's single-image slider path reallocates its parameters on every render.

## Claims rejected or narrowed after checking the surrounding code

| Focused-review claim | Source-grounded correction |
| --- | --- |
| `dt_dev_process_image_job` is an export-only entry point | False. `src/control/jobs/develop_jobs.c:28–49` calls it for the regular preview, second preview and interactive full pipe. Export has its own path in `src/imageio/imageio.c:1045`. |
| Reserve the IMAGE flag for an export pipe | Too simplistic. IMAGE is an additional helper-image flag; EXPORT is a separate pipe purpose. Fix caching only after reviewing flag-dependent behavior, including `finalscale`. |
| The adapter never enables colisa | False. At the reviewed revision, `om_engine_render` passed TRUE to `dt_dev_add_history_item_ext`; `_dev_add_history_item_ext` explicitly sets `module->enabled = TRUE` (`src/develop/develop.c`, enable branch near 1232). |
| GPU state is likely being read from the UI thread | Not true in the current wrapper. `omalux/native/main.cpp::Editor::run` performs both engine rendering and warning reads on the owning worker, then queues copied strings/images to Qt. A future threading change still needs synchronization review. |
| The returned backbuffer necessarily races with the next render | Not in the current serialized path: `Editor::run` immediately makes an owned `QImage::copy` before starting another render. This ownership contract must remain explicit. |
| `DT_DEVICE_NONE` might accidentally claim an already-locked device | Ruled out for this pin: `src/common/darktable.h:178–179` defines CPU = −1, NONE = −2; the claimed-device condition is `devid > DT_DEVICE_CPU`. |
| Missing-control failure leaves a live partially initialized dev forever | The C open function does leave cleanup to its caller; the current C++ failure path calls `om_engine_cleanup`. This is an API-contract concern, not a confirmed leak through our current caller. |

Do not replace GPU telemetry with a single different transient flag without tracing the full lifecycle. Configuration availability, runtime stopping, selected device, mixed CPU/GPU module execution and per-render fallback are distinct states. The primary [architecture analysis](darktable.md) includes the reconciled implementation priorities and runtime checks still needed.

The preset implementation now rebinds parameter pointers after style application and updates history in `om_engine_update_controls` only for explicit edits. Rendering alone no longer enables the module. The review above records the earlier source snapshot.
