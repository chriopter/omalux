# Distributing looks and camera presets for darktable

Everything Omalux ships can be used in plain darktable. darktable separates two things, and the bundle follows that split:

| darktable term | what it is | file | how a user installs it |
| --- | --- | --- | --- |
| Style | a look: settings of several modules | `.dtstyle` (XML) plus a `.cube` LUT where the look uses one | lighttable → *styles* → import |
| Module preset | settings of one module, optionally applied automatically by camera, lens, ISO or file type | `.dtpreset` (XML) | preferences → *presets* → import |
| Input profile | the camera's colour characterisation used by *input color profile* | `.icc` in the configuration's `color/in/` | copy the file, restart |

A "camera preset" in Omalux's words is therefore an input profile plus a module preset for `colorin` that selects it automatically for that camera. Looks never set the input profile, so both layers stay independent, as camera profiles and presets do in Lightroom.

## Building the bundle

```sh
python3 development/tools/darktable/build_bundle.py               # dist/darktable/, LUTs as files
python3 development/tools/darktable/build_bundle.py --embed-luts  # LUTs compressed into the styles (needs gmic)
```

The builder rewrites every style's name to `Omalux|<group>|<name>` so the looks appear as one tree in darktable's styles module. Group names come from the style folder: `film` → Film, `series/movie` → Movie, `series/late-summer` → Late Summer, `experimental` → Experimental, `monochrome` → Monochrome; `neutral` and `chromatic` sit directly under `Omalux`. A family named by `family.json` files (see [style bundles](styles.md#families-and-their-names)) keeps its levels: `dhh/kodak` → `Omalux|DHH|Kodak|<name>`, stored under `styles/dhh/kodak/`. The repository's own `style.dtstyle` files keep their short names; the hierarchy is added only in the bundle.

Styles reference their LUT by the catalogue-relative path (for example `film/film-chrome/look.cube`). Without `--embed-luts` the bundle copies the cubes to `luts/` in that layout, and the user sets darktable's *3D LUT root folder* to it. With `--embed-luts` the cube is compressed with G'MIC into up to 2048 colour keypoints and stored in the `lut3d` parameters, darktable's own mechanism for `.gmz` LUTs, so the style needs no external file. This is lossy for our cubes: at G'MIC error 2 the desert-signal cube came back with a mean deviation of 4.3 and a maximum of 76 in 8-bit units, which is more than the calibration tolerates. Use it only when an external LUT folder is impossible, and check the result; it also needs `gmic` on the path and a darktable built with G'MIC support (the Arch package is).

## Camera presets

Camera presets live in `catalog/camera/<maker>/<model>.dtpreset` (film profiles, which are presets too, have their own section below); a preset that selects an input profile keeps the profile beside it as `catalog/camera/<maker>/<model>.icc`. Omalux imports these presets into its own session and copies the profiles into darktable's `color/in`, so the app shows the same base as a darktable user who installed the bundle. A preset file is what darktable itself writes from preferences → presets → export:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<darktable_preset version="1.0">
  <preset>
    <name>Omalux camera profile</name>
    <description>Input profile for FUJIFILM X-T4</description>
    <operation>colorin</operation>
    <op_params>…hex…</op_params>       <!-- type = 0 (file), filename = "omalux-fujifilm-x-t4.icc" -->
    <op_version>7</op_version>
    <enabled>1</enabled>
    <autoapply>1</autoapply>
    <model>X-T4</model>
    <maker>FUJIFILM</maker>
    <lens>%</lens>
    <iso_min>0</iso_min> <iso_max>340282346638528859811704183484516925440</iso_max>
    <exposure_min>0</exposure_min> <exposure_max>340282346638528859811704183484516925440</exposure_max>
    <aperture_min>0</aperture_min> <aperture_max>340282346638528859811704183484516925440</aperture_max>
    <focal_length_min>0</focal_length_min> <focal_length_max>1000</focal_length_max>
    <blendop_params>…hex…</blendop_params>   <!-- default blending with the module's blend colour space -->
    <blendop_version>14</blendop_version>
    <multi_priority>0</multi_priority>
    <multi_name></multi_name>
    <multi_name_hand_edited>0</multi_name_hand_edited>
    <filter>0</filter>
    <def>0</def>
    <format>2</format>                 <!-- FOR_RAW; 1 = LDR, 4 = HDR -->
  </preset>
</darktable_preset>
```

`maker` and `model` are matched as SQL patterns against the EXIF maker and model, so `X-T%` covers a family.

A preset has to carry valid blending parameters, or darktable skips it when applying, and in them the blend colour space (`blend_cst`) darktable gives the module by default: 2 (Lab) for `sharpen` and `nlmeans`, 4 (RGB, scene-referred) for `rgblevels`, 0 for modules without blending (`lens`, `colorin`). With 0 on a module that blends, the preset still renders the same, because darktable fills the default in when it applies the preset; but the module then differs from the stored preset in that one field, and darktable neither ticks the preset in the module's preset menu nor names the module and its history step after it. `camera_presets.py` writes the value per module (`BLEND_CST`) and refuses a module it has no entry for; `camera_presets.py check` verifies the shipped files, and the regression suite runs it. No preset uses a mask, so the value does not change any pixel. `rgblevels` presets assume darktable's default scene-referred workflow; under the display-referred workflow darktable keeps the stored 4, where its own default would be 3.

The presets in the repository are generated by `development/tools/darktable/camera_presets.py generate` from its table. The first applies to every RAW file: capture sharpening (`sharpen`, amount 1.5, radius 2, threshold 2). darktable sharpens nothing by default, and the target renderings are sharper than ours on RAW files only; JPEG sources already match. Measured at full size on six RAW files under four styles, this setting brings detail and flat-area roughness closest to the targets together; the threshold keeps low-contrast noise out, which plain sharpening and the demosaicing presets of *diffuse or sharpen* both amplify. A second preset for every RAW file reduces colour noise only (`nlmeans`, luma 0, chroma 0.5): the targets keep fine luminance grain but no colour mottle, and removing luminance noise as well turned grass into mush. Styles carry no `sharpen` or `nlmeans` item of their own unless the look calls for it, because a style item overrides the camera preset even when it is switched off. The next set switches the *lens correction* module to *embedded metadata* for Fujifilm, Olympus, OM System, DJI, Google Pixel, Apple iPhone and Panasonic: those files carry distortion, vignetting and chromatic aberration data that the target renderings apply and darktable does not by default. Measured on the calibration corpus with the best per-image cube, the correction takes the iPhone image from ΔE 4.1 to 2.3, the DJI image from 2.9 to 1.8, the Pixel image from 4.4 to 3.7, the Fujifilm image from 5.5 to 4.7 and the Olympus image from 4.5 to 4.3; on the Ricoh GR III image the same correction made things worse, so Ricoh gets no preset. Sigma fp files get the same embedded correction. The Hasselblad L1D (the camera of a DJI drone) writes its vignetting correction into the DNG as a gain map in OpcodeList3, which the DNG specification marks mandatory and darktable 5.6 skips, so corners come out up to 2.4 times too dark; its preset adds darktable's manual vignette correction fitted to that gain map as read from the file (strength 0.625, radius 0.1, steepness 0.8) and sets `has_been_set`, without which darktable replaces the lens parameters by autodetected defaults and drops the manual vignette. Seven models also get an exposure preset: Canon EOS 6D +0.4 EV, Canon EOS 70D +0.1, Fujifilm X-T10 +0.5, Olympus E-M1 +0.5, DJI FC220 +0.4, Canon PowerShot SX100 IS -0.3, Apple iPhone XS -0.1. The Ricoh GR III, Google Pixel 6 Pro and Leica M9 measured at 0 and get none. RAW developers apply a baseline exposure by camera model and darktable does not; each value is the offset that fits all looks at once on that camera's photograph in the reference set, measured again after the looks were refitted on it and once more through rgb levels, so with one photograph per camera it is a first estimate. The preset uses rgb levels, which no look carries: linked channels, luminance-preserving, grey at the middle so the gamma stays 1, and the white point at 2^-EV, which makes it a pure gain that extends above white. A second instance of the exposure module was tried first and failed in darktable: when a look was applied, its own exposure instance ended up where the calibration had not assumed it, and looks with an exposure of their own came out darker; checked with the Omalux engine against the calibration renders, rgb levels matches within grain noise. Panasonic was first excluded on the same measurement, taken while the presets were silently not applied at all; with them applied, the TZ60 frame loses its dark corners and distortion and matches the target, although the score barely moves because the corrected framing no longer coincides pixel for pixel.

### Input profiles

The second kind of camera preset is an ICC input profile plus a `colorin` preset that selects it. `development/tools/darktable/camera_profile.py` writes both:

```sh
python3 development/tools/darktable/camera_profile.py --camera "FUJIFILM X-T10" \
    --name "Fujifilm X-T10" --out catalog/camera/fujifilm/x-t10.icc --preset
```

It reads the camera's colour matrix from rawspeed's `data/cameras.xml` in the darktable submodule (the numbers behind darktable's "standard color matrix"), inverts it, maps camera white onto the ICC connection white point D50 and writes a v2 matrix/TRC input profile with linear tone curves. `--matrix` takes nine values instead, so a measured or DCP-derived characterisation can be used the same way.

`catalog/camera/fujifilm/x-t10.icc` is included as a worked example. Its preset does **not** apply itself: a profile rebuilt from the camera matrix is a starting point, not a measurement, and it does not reproduce darktable's own matrix path exactly (measured on one file: about 3 ΔE, because darktable combines that matrix with its own white balance and colour calibration chain). Import it and pick it in the *input color profile* module to compare; generate with `--autoapply` for a profile that should replace darktable's choice for a camera.

Adobe DCP files must be converted first (for example with DCamProf), and only profiles whose licence allows redistribution belong in the repository.

The calibration renderer applies the same presets to RAW inputs by camera (`DT_CAMERA_PRESETS=0` disables that), so the looks are fitted on the base a darktable user with the presets installed will see.

## Film profiles

A film profile is a lookup table for darktable's *LUT 3D* module plus a module preset that names it. The catalogue ships the film profiles of the DHH set, converted to lookup tables: 89 film stocks, most in the two variants of the set (`C` and `L`), 176 tables. They are independent of the camera and work on RAW and JPEG files. Each table maps sRGB to sRGB and belongs where a look's lookup table sits, after the tone mapping; that is where darktable places LUT 3D.

### Layout

```
catalog/camera/<group>/<film>/film.json        what the film is and which tables it has
catalog/camera/<group>/<film>/<variant>.png    the lookup table of a variant (HALD image; .cube and .3dl work too)
catalog/camera/<group>/<film>/<variant>.dtpreset   the LUT 3D preset naming that table; generated
catalog/camera/<group>/<film>/<variant>.jpg    optional preview: the table applied to the shared beach photograph
```

`<group>` is `dhh`, `<film>` the film stock in lower case with `-` between words (`kodak-portra-400`, `portra-800-plus-1`), `<variant>` the variant key in lower case (`c`, `l`).

`film.json`:

```json
{
  "version": 1,
  "group": "DHH",
  "brand": "Kodak",
  "name": "Kodak Portra 400",
  "colorspace": "srgb",
  "interpolation": "tetrahedral",
  "variants": [
    {"key": "C", "name": "Kodak Portra 400 2C", "lut": "c.png", "id": "film-kodak-portra-400-2c"},
    {"key": "L", "name": "Kodak Portra 400 L", "lut": "l.png", "id": "film-kodak-portra-400-l"}
  ]
}
```

- `group` is the heading in Omalux, written as it should read (`DHH`); `brand` the sub-heading; `name` the film stock, one row.
- `colorspace` is LUT 3D's *application color space*: `srgb`, `adobergb`, `rec709`, `linear-rec709`, `linear-rec2020` or `linear-prophoto`; `interpolation` is `tetrahedral`, `trilinear` or `pyramid`.
- A variant has the `key` shown on its chip, the `name` of the film as the set writes it (shown in History and in tooltips), its `lut` file beside `film.json`, and an `id` by which a look can name its film (`"film"` in the look's `style.json`).
- None of these texts may contain ` · `, which separates them in the preset.

The preset (`<variant>.dtpreset`) is an ordinary darktable preset file as above, with:

| field | value |
| --- | --- |
| `name` | `Omalux <group>: <variant name>` (`Omalux DHH: Kodak Portra 400 2C`); unique |
| `description` | `<group> · <brand> · <film> · <key> · <id>`; Omalux reads group, brand, film stock, variant and id from it |
| `operation`, `op_version` | `lut3d`, 3 |
| `op_params` | `dt_iop_lut3d_params_t`: `filepath` = `camera/<group>/<film>/<lut>`, colour space, interpolation, no compressed table. Written in darktable's compressed form (`gz…`), because the parameters are 13 kB of mostly zeros |
| `blendop_params` | default blending with blend colour space 4 (RGB, scene-referred), as for every preset here |
| `multi_name`, `multi_name_hand_edited` | the variant name, 0: the module instance and its history step are named after the film, and another film renames it |
| `autoapply`, `maker`, `model`, `format`, `filter` | 0, `%`, `%`, 0, 0: never applied automatically, offered for every image |

`development/tools/darktable/film_profiles.py` writes and checks all of this:

```sh
python3 development/tools/darktable/film_profiles.py install <converted films> [catalog/camera] [--cube]
python3 development/tools/darktable/film_profiles.py generate    # film.json + tables -> .dtpreset
python3 development/tools/darktable/film_profiles.py check       # presets agree with film.json; the regression suite runs it
```

`install` takes a folder with one sub-folder per converted film, each holding `look.cube`, a `style.json` with `name` (the film as the set writes it, variant included), `group`, `family` (the variant key) and `kind: "film"`, and optionally a `style.dtstyle` whose `lut3d` item gives colour space and interpolation. The sub-folder's name becomes the variant's `id`. Variants of one film are found by their name without the trailing key (`… - C`, `… C`, `… 2C`, `… L`); the brand comes from the name's first words. The catalogue folder of each installed film is rewritten.

### HALD images instead of cubes

A 33-node cube as text is about 950 kB; `install` stores each table as a 16-bit HALD image of level 6 (36 nodes per axis, 216 × 216 pixels, about 190 kB, 36 MB for the set), which LUT 3D reads directly. The cube is resampled at the 36 nodes with tetrahedral interpolation, the way LUT 3D reads a cube by default. Rendered with the Omalux engine on the beach photograph, three films (Kodak Portra 400, Fuji Velvia 50, Kodak TRI-X 400) differed between HALD image and cube by 0.04 to 0.07 of 255 on average. `--cube` keeps the cube files.

### The LUT root folder

darktable has one *3D LUT root folder* (`plugins/darkroom/lut3d/def_path`), and every LUT 3D file name is relative to it. Looks name their table relative to the style catalogue (`film/film-chrome/look.cube`), film profiles as `camera/<group>/<film>/<lut>`. The root therefore has to hold the style catalogue's folders and the camera catalogue as `camera`:

- **Omalux** builds that folder per session (`omalux/style_assets.py`, `prepare_lut_root`): links to every entry of `catalog/styles/` (and to `my-styles`, where saved looks go) plus `camera` → `catalog/camera/`. A style family must not be called `camera`.
- **The darktable bundle** has it as `luts/`: the looks' tables in the catalogue layout and the films' tables under `luts/camera/dhh/…`. A darktable user sets *preferences → processing → 3D LUT root folder* to the bundle's `luts/` folder once; looks and films both resolve from it.

In plain darktable a film is a preset of LUT 3D (preset menu of the module). To use a film together with a look there, create a second instance of LUT 3D, move it before the first, and apply the film preset to it; the look's style then lands on the other instance. Omalux does this by itself (see `controls.md`, Film profiles).

### Placeholder films for tests

`omalux/tests/camera/dhh/` holds three made-up films in this layout (`film_profiles.py placeholders omalux/tests/camera`); the regression suite adds them to a copy of the camera catalogue. They are not catalogue content.

## What the bundle does not do

It does not change darktable's defaults, workflow or module order, and it does not install anything by itself. Looks applied in darktable behave exactly as in Omalux only when the same darktable release renders them; the parameter layouts target 5.6.
