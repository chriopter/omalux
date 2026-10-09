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


def check_mailbox(mailbox, operations):
    """Generic module edits reach split mode as one current single-module style each."""
    lines = [line.split() for line in mailbox.read_text().splitlines() if line.startswith('module ')]
    seen = [line[3] for line in lines]
    for operation in operations:
        if seen.count(operation) != 1:
            raise RuntimeError(f'Mailbox should carry one {operation} snapshot: {seen}')
    for line in lines:
        style = (mailbox.parent / (line[4] + '.dtstyle')).read_text()
        if f'<operation>{line[3]}</operation>' not in style:
            raise RuntimeError(f'Snapshot {line[4]} does not contain {line[3]}')
    print('Mailbox carries single-module snapshots for', ', '.join(sorted(operations)), flush=True)


def check_blend_mailbox(mailbox):
    """Blend edits and extra instances reach split mode in the module's snapshot."""
    text = mailbox.read_text()
    lines = {line.split()[3]: line.split()[4] for line in text.splitlines() if line.startswith('module ')}
    # Deleting and moving instances sends the whole history as an XMP sidecar (a new epoch), which
    # then carries the earlier colour balance edits.
    # A later epoch (the style applied at the end) replaces the journal line, the files stay.
    sidecars = sorted(mailbox.parent.glob('omalux-sidecar-*.xmp'), key=lambda p: int(p.stem.rsplit('-', 1)[1]))
    if not sidecars:
        raise RuntimeError('Deleting or moving an instance should send a sidecar')
    sidecar = sidecars[-1].read_text()
    history = [line.split()[2] for line in text.splitlines() if line.startswith('history ')]
    for operation in ('exposure', 'colorbalancergb'):
        if (operation not in lines and f'darktable:operation="{operation}"' not in sidecar
                and not history):
            raise RuntimeError(f'Mailbox should carry a {operation} snapshot: {sorted(lines)}')
    style = (mailbox.parent / (lines['exposure'] + '.dtstyle')).read_text()
    if '<multi_priority>1</multi_priority>' not in style:
        raise RuntimeError('The exposure snapshot does not carry its second instance')
    print('Mailbox carries blend edits and the second exposure instance', flush=True)


def check_sidecar(mailbox):
    """Drawn shapes reach split mode as an XMP sidecar of the whole history."""
    lines = [line.split() for line in mailbox.read_text().splitlines() if line.startswith('sidecar ')]
    if len(lines) != 1:
        raise RuntimeError(f'Mailbox should carry one current sidecar: {mailbox.read_text()[:400]}')
    xmp = (mailbox.parent / (lines[0][2] + '.xmp')).read_text()
    for needle in ('darktable:masks_history', 'retouch', 'exposure'):
        if needle not in xmp:
            raise RuntimeError(f'Sidecar {lines[0][2]} lacks {needle}')
    print('Mailbox carries the drawn shapes as an XMP sidecar', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--split', action='store_true', help='also verify the GTK comparison path')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='omalux-regression-') as folder:
        work = Path(folder)
        # QML component and keyboard tests first: fast, no engine needed.
        for test in sorted((ROOT / 'omalux/tests/components').glob('tst_*.qml')):
            print('Running', test.name, flush=True)
            env = dict(os.environ, QT_QPA_PLATFORM='offscreen', XDG_CONFIG_HOME=str(work / 'config-qml'),
                       QML_XHR_ALLOW_FILE_READ='1')  # tst_panes reads the generated layout
            result = subprocess.run(['/usr/lib/qt6/bin/qmltestrunner', '-input', str(test)], cwd=ROOT, env=env,
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=240)
            if result.returncode:
                raise RuntimeError(result.stdout[-12000:])
            print(next((line for line in result.stdout.splitlines() if line.startswith('Totals')), ''), flush=True)
        shutil.copytree(ROOT / 'catalog/styles', work / 'styles')
        # A G'MIC compressed LUT file with two LUTs below the LUT root (module-tools-rest.json).
        # Always with an explicit output and without a display: gmic must never open a window.
        gmic_env = {k: v for k, v in os.environ.items() if k not in ('DISPLAY', 'WAYLAND_DISPLAY')}
        gmic_env['QT_QPA_PLATFORM'] = 'offscreen'
        subprocess.run(['gmic', '-v', '-1', '/usr/share/gmic/gmic_cluts.gmz', 'k[0,1]', '-o',
                        str(work / 'styles' / 'two-luts.gmz')], check=True, env=gmic_env,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        env = os.environ.copy()
        env.update(QT_QPA_PLATFORM='offscreen', QT_FORCE_STDERR_LOGGING='1',
                   OMALUX_STYLES_DIR=str(work / 'styles'))
        for key in ('OMALUX_CAPTURE', 'OMALUX_PREVIEW_DIR', 'OMALUX_FLUSH_PREVIEW_CACHE'):
            env.pop(key, None)
        workflow = [
            {'controls': {'exposure': .35, 'grain': 30, 'temperature': 5500, 'tint': 1.01}},
            {'control': 'denoise_mode', 'value': 1},
            {'control': 'denoise_4_3', 'value': .6},
            {'halation': True},
            {'control': 'diffuse_enabled', 'value': 0},
            {'style': 'film/film-chrome/style.dtstyle'},
            {'control': 'lut_opacity', 'value': 50},
            {'saveStyle': 'Omalux regression workflow'},
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
                   ROOT / 'omalux/tests/style-hover.json', ROOT / 'omalux/tests/module-parameters.json',
                   ROOT / 'omalux/tests/blending.json', ROOT / 'omalux/tests/module-tools.json',
                   ROOT / 'omalux/tests/module-tools-ui.json',
                   ROOT / 'omalux/tests/keyboard.json', ROOT / 'omalux/tests/pointer.json',
                   ROOT / 'omalux/tests/canvas-engine.json', ROOT / 'omalux/tests/canvas.json',
                   ROOT / 'omalux/tests/canvas-picker.json',
                   ROOT / 'omalux/tests/crop.json',
                   work / 'workflow.json']
        # Displayed conversions, runtime lists and file choices; "{WORK}" names this run's folder.
        values = work / 'module-values.json'
        values.write_text((ROOT / 'omalux/tests/module-values.json').read_text().replace('{WORK}', str(work)))
        scripts.append(values)
        # Area E: the remaining pickers, tone equalizer, color mapping across two images ...
        for name in ('module-tools-rest.json', 'module-tools-rest-ui.json', 'module-display.json'):
            script = work / name
            script.write_text((ROOT / 'omalux/tests' / name).read_text().replace('{WORK}', str(work)))
            scripts.append(script)
        # The shell around the panes: toolbar, search, dialogs, Info, and files that cannot be
        # opened. shell-empty.json starts with such a file and opens a photograph from there.
        (work / 'broken.jpg').write_text('not an image\n')
        for name in ('shell.json',) + (() if args.split else ('shell-empty.json',)):
            script = work / name
            script.write_text((ROOT / 'omalux/tests' / name).read_text().replace('{WORK}', str(work))
                              .replace('{ROOT}', str(ROOT)))
            scripts.append(script)
        # Escape on the open dialog and the keys back in the editor: Qt's own file dialog (no
        # platform theme) and, on a headless GTK display only (broadway, never a visible window),
        # the GTK file chooser Qt's gtk3 theme opens (Omarchy's QT_QPA_PLATFORMTHEME).
        scripts.append(ROOT / 'omalux/tests/dialog-quick.json')
        if os.environ.get('GDK_BACKEND') == 'broadway':
            scripts.append(ROOT / 'omalux/tests/dialog-native.json')
        else:
            print('Skipping dialog-native.json: needs GDK_BACKEND=broadway (headless GTK display)', flush=True)
        values_mailbox = work / 'mailbox-values' / 'controls'
        values_mailbox.parent.mkdir()
        mailbox = work / 'mailbox' / 'controls'
        mailbox.parent.mkdir()
        blend_mailbox = work / 'mailbox-blending' / 'controls'
        blend_mailbox.parent.mkdir()
        canvas_mailbox = work / 'canvas-mailbox' / 'controls'
        canvas_mailbox.parent.mkdir()
        used_opencl = False
        for script in scripts:
            env['XDG_CONFIG_HOME'] = str(work / ('config-' + script.stem))
            env['OMALUX_SMOKE_SCRIPT'] = str(script)
            env.pop('OMALUX_RECORD_MAILBOX', None)
            # shell.json closes the export file dialog with Escape: Qt's own dialog, as dialog-quick.
            env['QT_QPA_PLATFORMTHEME'] = {'dialog-quick': '', 'dialog-native': 'gtk3', 'shell': '',
                                           'shell-empty': ''}.get(
                script.stem, os.environ.get('QT_QPA_PLATFORMTHEME', ''))
            if script.stem == 'module-parameters' and not args.split:
                env['OMALUX_RECORD_MAILBOX'] = str(mailbox)
            if script.stem == 'module-values' and not args.split:
                env['OMALUX_RECORD_MAILBOX'] = str(values_mailbox)
            if script.stem == 'blending' and not args.split:
                env['OMALUX_RECORD_MAILBOX'] = str(blend_mailbox)
            if script.stem == 'canvas-engine' and not args.split:
                env['OMALUX_RECORD_MAILBOX'] = str(canvas_mailbox)
            command = ROOT / ('development/start_split' if args.split else 'development/start')
            log = work / (script.stem + '.log')
            print('Running', script.name, flush=True)
            with log.open('w') as output:
                image = [str(work / 'broken.jpg')] if script.stem == 'shell-empty' else []
                result = subprocess.run([str(command), *image], cwd=ROOT, env=env, stdout=output,
                                        stderr=subprocess.STDOUT, timeout=240)
            text = log.read_text()
            if result.returncode or 'Smoke complete' not in text:
                raise RuntimeError(text[-12000:])
            used_opencl = used_opencl or 'OpenCL auto' in text
            for line in text.splitlines():
                if 'Drag draft frames' in line or 'Smoke complete' in line or line.startswith('Parameter ') \
                        or line.startswith('Instances ') or 'Rejected as expected' in line \
                        or line.startswith('Pixels '):
                    print(line, flush=True)
        # The launcher keeps the compiled OpenCL kernels between launches (omalux/dev.py): after a
        # run on a GPU the store holds them, so the next start does not compile again.
        store = Path(env.get('OMALUX_KERNEL_CACHE') or Path(env.get('XDG_CACHE_HOME') or Path.home() / '.cache')
                     / 'omalux' / 'opencl-kernels')
        if used_opencl and not any('kernels_for_' in entry.name for entry in store.iterdir()):
            raise RuntimeError(f'No OpenCL kernels kept in {store}')
        if not args.split:
            check_mailbox(mailbox, {'exposure', 'tonecurve', 'rgbcurve'})
            check_mailbox(values_mailbox, {'colorbalance', 'channelmixerrgb', 'colorharmonizer', 'splittoning',
                                           'colorchecker', 'colorin', 'lens', 'lut3d'})
            check_blend_mailbox(blend_mailbox)
            check_sidecar(canvas_mailbox)
        for name, size in [('full.png', '1536x1024'), ('square.jpg', '1024x1024')]:
            actual = subprocess.check_output(['magick', 'identify', '-format', '%wx%h',
                                              str(work / name)], text=True)
            if actual != size:
                raise RuntimeError(f'{name}: expected {size}, got {actual}')
        manifests = list((work / 'bundle').rglob('style.json'))
        if len(manifests) != 1:
            raise RuntimeError('Exported style bundle missing')
        bundle = manifests[0].parent
        manifest = json.loads(manifests[0].read_text())
        if not manifest.get('assets'):
            raise RuntimeError('Exported LUT dependency missing')
        for name in ['style.dtstyle', 'thumbnail.jpg', *[asset['path'] for asset in manifest['assets']]]:
            if not (bundle / name).is_file():
                raise RuntimeError(f'Bundle asset missing: {name}')
        print('All real-engine regressions passed; PNG/JPEG dimensions and bundled LUT verified.', flush=True)


if __name__ == '__main__':
    main()
