#!/usr/bin/env python3
"""Build and run the native Qt/darktable prototype in a private session."""
import argparse
import json
from preset_assets import prepare_assets
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('image', nargs='?')
    parser.add_argument('--input', dest='input_image')
    parser.add_argument('--split', action='store_true')
    args = parser.parse_args()
    if args.image and args.input_image:
        parser.error('Use either an image argument or --input, not both')
    source = Path(args.input_image or args.image or ROOT / 'assets/images/beach-volleyball.jpg').resolve()
    if not source.is_file():
        parser.error(f'Image does not exist: {source}')
    binary = ROOT / 'omalux/build/omalux'
    library = Path(os.environ.get('DARKTABLE_LIBRARY', '/usr/lib/darktable/libdarktable.so')).resolve()
    if not library.is_file():
        parser.error(f'darktable library not found: {library}')
    native_sources = list((ROOT / 'omalux/native').glob('*')) + [ROOT / 'omalux/build-native.sh', library]
    marker = ROOT / 'omalux/build/library-path'
    if not binary.exists() or not marker.exists() or marker.read_text().strip() != str(library) or any(p.stat().st_mtime > binary.stat().st_mtime for p in native_sources):
        subprocess.run([str(ROOT / 'omalux/build-native.sh')], check=True)
    # Mesa Rusticl requires explicit driver opt-in; respect user overrides.
    os.environ.setdefault("RUSTICL_ENABLE", "radeonsi")
    os.environ.setdefault("OMALUX_PRESETS_DIR", str(ROOT / "presets"))
    children = []
    with tempfile.TemporaryDirectory(prefix='omalux-dev-') as folder:
        session = Path(folder)
        dt = os.environ.get('DARKTABLE_BIN', '/usr/bin/darktable')
        lut_config = 'plugins/darkroom/lut3d/def_path=' + str(Path(os.environ['OMALUX_PRESETS_DIR']).resolve())
        dtargs = [dt, '--conf', lut_config, '--configdir', str(session / 'config'), '--cachedir', str(session / 'cache'),
                  '--library', str(session / 'library.db'), '--moduledir', os.environ.get('DARKTABLE_MODULEDIR','/usr/lib/darktable'),
                  '--datadir', os.environ.get('DARKTABLE_DATADIR','/usr/share/darktable'),
                  '--conf', 'write_sidecar_files=never', '--conf', 'show_splash_screen=false',
                  '--conf', 'ui/show_welcome_screen=false']
        (session / 'config').mkdir()
        configs = [session / 'config']
        if args.split:
            configs.append(session / 'comparison')
            configs[-1].mkdir()
        asset_errors = prepare_assets(Path(os.environ['OMALUX_PRESETS_DIR']).resolve(), configs)
        ui_env = os.environ.copy()
        ui_env['OMALUX_PRESET_ASSET_ERRORS'] = json.dumps(asset_errors)
        ui_env.pop('OMALUX_COMPARISON_MAILBOX', None)
        try:
            if args.split:
                other = session / 'comparison'
                mailbox = session / 'controls'
                ui_env['OMALUX_COMPARISON_MAILBOX'] = str(mailbox)
                (other / 'luarc').write_text((ROOT / 'omalux/comparison.lua').read_text())
                comparison_env = os.environ.copy()
                comparison_env['OMALUX_COMPARISON_MAILBOX'] = str(mailbox)
                children.append(subprocess.Popen([dt, str(source), '-d', 'lua', '--conf', lut_config, '--configdir', str(other),
                    '--cachedir', str(other / 'cache'), '--library', str(other / 'library.db'),
                    '--conf', 'write_sidecar_files=never', '--conf', 'show_splash_screen=false',
                    '--conf', 'ui/show_welcome_screen=false'], env=comparison_env))
            ui = subprocess.Popen([str(binary), str(source), str(ROOT / 'assets'), *dtargs], env=ui_env)
            children.append(ui)
            return ui.wait()
        finally:
            for child in reversed(children):
                if child.poll() is None:
                    child.terminate()
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        child.kill(); child.wait()


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit(130)
