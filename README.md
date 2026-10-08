# Omalux

A keyboard-friendly interface for [darktable](https://www.darktable.org/), built for Omarchy. Our work is the UI; darktable provides the photo engine. Thanks to its developers and contributors. This is an independent project, not an official darktable edition.

[Website](https://omalux.org) · [Releases](https://github.com/chriopter/omalux/releases) · [Documentation](development/docs/README.md)

![Omalux development preview](https://omalux.org/app-screenshot-dark.png)

## Run

**Early development, source only.** Requires darktable 5.6.0 or 5.6.1, Qt 6 and build tools. See [setup requirements](development/docs/setup.md#development-scripts).

```sh
git clone --recurse-submodules https://github.com/chriopter/omalux.git
cd omalux
development/start
```

Sessions are temporary: export your photos or save styles before closing.

## Keyboard

The sidebar keeps one selection, marked in the accent colour; the bottom bar shows the keys that apply to it and `?` lists all of them.

| Keys | Action |
| --- | --- |
| `↑` / `↓` (`K` / `J`), `Home` / `End` | Previous / next item in the visible pane, in displayed order across modules; first / last |
| `Page Up` / `Page Down` | Previous / next module or style group |
| `←` / `→` (`H` / `L`) | Adjust the selected slider by one darktable step, choose the previous / next option, toggle a switch, close / open a module |
| `Shift` + `←` / `→`, `Ctrl` or `Alt` + `←` / `→` | Ten steps, a tenth of a step |
| `Enter` / `Space` | Activate: open or close a module, apply a style, restore a history step, press a button, open a colour picker; on a curve or graph, its own arrow keys edit the points until `Esc` |
| `R`, `Shift+R`, `E` | Reset the selected parameter, reset its whole module, switch the module on/off |
| `1` … `9`, `Tab` / `Shift+Tab` (`]` / `[`) | Pane by its place in the tab strip (Filters, Styles, Tone, Color, Detail, Effects, Crop & Rotate, History, Info), next / previous pane |
| `/` or `Ctrl+F` | Search modules and controls; `Esc` clears the search and leaves it, `Enter` or `↓` keep it and go to the first result. The style search is an item in the Styles list |
| `G` / `S` / `M`, `A` | Grain strength / coarseness / mid-tones bias (opening grain details), grain details |
| `+` / `−`, `0`, `F` | Zoom, fit, photograph fullscreen (`F` or `Esc` to leave) |
| `Enter` / `Esc` while cropping | Apply / cancel the crop |
| `O` / `Ctrl+O`, `Ctrl+S`, `?` / `F1` | Open, export, keyboard reference |

Text fields keep every key while you type. Clicking a control selects it for the keyboard without taking the keys away; after a dialog closes, keys work again at once.

## Development

Start scripts live in `development/`, calibration tools in `development/tools/calibration/`, and documentation in `development/docs/`.

- `development/start [image]` — start Omalux; defaults to the beach photo.
- `development/start_split [image]` — also open darktable for comparison.
- `development/style_preview <folder>` / `--all` — regenerate style thumbnails.
- `development/update` — update the darktable submodule to the latest stable release; does not commit or push.

[Development guide](development/docs/setup.md) · [Engine architecture](development/docs/architecture/darktable.md) · [Original v0 release](https://github.com/chriopter/omalux/releases/tag/v0.2.0)

## TODO

- [ ] Calibrate styles against their intended appearance ([details](development/docs/reference/styles.md#one-time-v0-conversion)).
