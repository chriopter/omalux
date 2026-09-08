#!/usr/bin/env python3
"""Exercise the real Qt/darktable workflow in disposable development sessions."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--split', action='store_true', help='also verify the GTK comparison path')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='omalux-regression-') as folder:
        work = Path(folder)
        shutil.copytree(ROOT / 'presets', work / 'presets')
        env = os.environ.copy()
        env.update(QT_QPA_PLATFORM='offscreen', QT_FORCE_STDERR_LOGGING='1',
                   OMALUX_PRESETS_DIR=str(work / 'presets'))
        for key in ('OMALUX_CAPTURE', 'OMALUX_PREVIEW_DIR', 'OMALUX_FLUSH_PREVIEW_CACHE'):
            env.pop(key, None)
        workflow = [
            {'controls': {'exposure': .35, 'grain': 30, 'temperature': 5500, 'tint': 1.01}},
            {'control': 'denoise_mode', 'value': 1},
            {'control': 'denoise_4_3', 'value': .6},
            {'halation': True},
            {'control': 'diffuse_enabled', 'value': 0},
            {'preset': 'film/film-chrome/preset.dtstyle'},
            {'control': 'lut_opacity', 'value': 50},
            {'savePreset': 'Omalux regression workflow'},
            {'applyNamed': 'Omalux regression workflow'},
            {'export': str(work / 'full.png')},
            {'panel': 2},
            {'geometry': 'begin', 'ratio': 1},
            {'geometry': 'apply'},
            {'export': str(work / 'square.jpg')},
            {'check': {'crop_left': 16.66667, 'crop_right': 16.66667, 'crop_enabled': 1}},
            {'geometry': 'begin'},
            {'geometry': 'cancel'},
            {'check': {'crop_enabled': 1}},
            {'exportNamed': 'Omalux regression workflow', 'destination': str(work / 'bundle')},
            {'deleteNamed': 'Omalux regression workflow'},
            {'open': str(work / 'square.jpg')},
            {'capture': str(work / 'reopened.png')},
        ]
        (work / 'workflow.json').write_text(json.dumps(workflow))
        scripts = [ROOT / 'omalux/tests/interactive-preview.json',
                   ROOT / 'omalux/tests/preset-hover.json', work / 'workflow.json']
        for script in scripts:
            env['XDG_CONFIG_HOME'] = str(work / ('config-' + script.stem))
            env['OMALUX_SMOKE_SCRIPT'] = str(script)
            command = ROOT / ('bin/dev_split' if args.split else 'bin/dev')
            log = work / (script.stem + '.log')
            print('Running', script.name, flush=True)
            with log.open('w') as output:
                result = subprocess.run([str(command)], cwd=ROOT, env=env, stdout=output,
                                        stderr=subprocess.STDOUT, timeout=240)
            text = log.read_text()
            if result.returncode or 'Smoke complete' not in text:
                raise RuntimeError(text[-12000:])
            for line in text.splitlines():
                if 'Drag draft frames' in line or 'Smoke complete' in line:
                    print(line, flush=True)
        for name, size in [('full.png', '1536x1024'), ('square.jpg', '1024x1024')]:
            actual = subprocess.check_output(['magick', 'identify', '-format', '%wx%h',
                                              str(work / name)], text=True)
            if actual != size:
                raise RuntimeError(f'{name}: expected {size}, got {actual}')
        manifests = list((work / 'bundle').rglob('preset.json'))
        if len(manifests) != 1:
            raise RuntimeError('Exported preset bundle missing')
        bundle = manifests[0].parent
        manifest = json.loads(manifests[0].read_text())
        if not manifest.get('assets'):
            raise RuntimeError('Exported LUT dependency missing')
        for name in ['preset.dtstyle', 'thumbnail.jpg', *[asset['path'] for asset in manifest['assets']]]:
            if not (bundle / name).is_file():
                raise RuntimeError(f'Bundle asset missing: {name}')
        print('All real-engine regressions passed; PNG/JPEG dimensions and bundled LUT verified.', flush=True)


if __name__ == '__main__':
    main()
