#!/usr/bin/env python3
"""Build and run the native Qt/darktable prototype in a private session."""
import argparse
import json
from style_assets import prepare_assets, prepare_camera_profiles, prepare_lut_root
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def kernel_store():
    """Where compiled OpenCL kernels are kept between launches."""
    base = os.environ.get('OMALUX_KERNEL_CACHE') or os.path.join(
        os.environ.get('XDG_CACHE_HOME') or os.path.expanduser('~/.cache'), 'omalux', 'opencl-kernels')
    return Path(base)


def link_kernels(cache):
    """Offer the kept kernel folders to a session's private cache directory.

    darktable compiles its OpenCL programs on first use and keeps the binaries in the cache
    directory, one folder per device and driver version. A fresh cache directory per launch
    made every start compile them again: about six seconds before the first image, against
    under one with the binaries. Only these folders are shared; everything else in the cache
    stays private to the session."""
    cache.mkdir(parents=True, exist_ok=True)
    store = kernel_store()
    try:
        store.mkdir(parents=True, exist_ok=True)
        for folder in store.iterdir():
            if folder.is_dir() and 'kernels_for_' in folder.name and not (cache / folder.name).exists():
                (cache / folder.name).symlink_to(folder, target_is_directory=True)
    except OSError as error:
        print(f'OpenCL kernel cache unavailable ({error}); kernels are compiled for this session')


def keep_kernels(cache):
    """Keep the kernel folders a session compiled for a device seen for the first time."""
    store = kernel_store()
    try:
        for folder in cache.iterdir():
            if folder.is_dir() and not folder.is_symlink() and 'kernels_for_' in folder.name \
                    and not (store / folder.name).exists():
                # Copy, then rename into place: another session may keep the same folder.
                staged = store / f'.{folder.name}.{os.getpid()}'
                shutil.copytree(folder, staged, symlinks=True)
                try:
                    staged.rename(store / folder.name)
                except OSError:
                    shutil.rmtree(staged, ignore_errors=True)
    except OSError:
        pass


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
    native_sources = [p for p in (ROOT / 'omalux/native').rglob('*') if p.is_file()] + list((ROOT / 'omalux/shaders').glob('*.frag')) + [ROOT / 'omalux/build-native.sh', library]
    marker = ROOT / 'omalux/build/library-path'
    if not binary.exists() or not marker.exists() or marker.read_text().strip() != str(library) or any(p.stat().st_mtime > binary.stat().st_mtime for p in native_sources):
        subprocess.run([str(ROOT / 'omalux/build-native.sh')], check=True)
    # Mesa Rusticl requires explicit driver opt-in; respect user overrides.
    os.environ.setdefault("RUSTICL_ENABLE", "radeonsi")
    os.environ.setdefault("OMALUX_STYLES_DIR", str(ROOT / "catalog/styles"))
    os.environ.setdefault("OMALUX_CAMERA_DIR", str(ROOT / "catalog/camera"))
    os.environ.setdefault("OMALUX_DESIGN_DIR", str(ROOT / "omalux/design"))
    children = []
    with tempfile.TemporaryDirectory(prefix='omalux-dev-') as folder:
        session = Path(folder)
        dt = os.environ.get('DARKTABLE_BIN', '/usr/bin/darktable')
        # One LUT root for looks (style catalogue) and film profiles (camera catalogue).
        lut_config = 'plugins/darkroom/lut3d/def_path=' + str(prepare_lut_root(
            Path(os.environ['OMALUX_STYLES_DIR']).resolve(), os.environ.get('OMALUX_CAMERA_DIR'),
            session / 'luts'))
        performance_args = [
            '--conf', 'opencl_fast=false',
            '--conf', 'resourcelevel=' + os.environ.get('OMALUX_RESOURCES', 'default'),
            '--conf', 'opencl_scheduling_profile=' + os.environ.get('OMALUX_GPU_PROFILE', 'default' if args.split else 'very fast GPU'),
        ]
        dtargs = [dt, '--conf', lut_config, '--configdir', str(session / 'config'), '--cachedir', str(session / 'cache'),
                  '--library', str(session / 'library.db'), '--moduledir', os.environ.get('DARKTABLE_MODULEDIR','/usr/lib/darktable'),
                  '--datadir', os.environ.get('DARKTABLE_DATADIR','/usr/share/darktable'),
                  '--conf', 'write_sidecar_files=never', '--conf', 'show_splash_screen=false',
                  '--conf', 'ui/show_welcome_screen=false', *performance_args]
        (session / 'config').mkdir()
        configs = [session / 'config']
        caches = [session / 'cache']
        if args.split:
            configs.append(session / 'comparison')
            configs[-1].mkdir()
            caches.append(session / 'comparison' / 'cache')
        for cache in caches:
            link_kernels(cache)
        asset_errors = prepare_assets(Path(os.environ['OMALUX_STYLES_DIR']).resolve(), configs)
        prepare_camera_profiles(os.environ.get('OMALUX_CAMERA_DIR'), configs)
        ui_env = os.environ.copy()
        ui_env['OMALUX_STYLE_ASSET_ERRORS'] = json.dumps(asset_errors)
        ui_env.pop('OMALUX_COMPARISON_MAILBOX', None)
        # Tests may record what split mode would send, without a comparison window.
        if not args.split and os.environ.get('OMALUX_RECORD_MAILBOX'):
            ui_env['OMALUX_COMPARISON_MAILBOX'] = os.environ['OMALUX_RECORD_MAILBOX']
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
                    '--conf', 'ui/show_welcome_screen=false', *performance_args], env=comparison_env))
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
            for cache in caches:
                keep_kernels(cache)


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit(130)
