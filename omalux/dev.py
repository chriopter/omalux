#!/usr/bin/env python3
"""Launch the minimal UI with a private darktable session (Linux development)."""
import ctypes
import ctypes.util
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time


def engine(arguments):
    # Internal darktable ABI: dt_init(argc, argv, init_gui, load_data, lua_state).
    # Tested with 5.6.0; keep this development shim in sync with upstream.
    library = os.environ.get('DARKTABLE_LIBRARY') or ctypes.util.find_library('darktable')
    if not library:
        library = next((str(p) for p in [Path('/usr/lib/darktable/libdarktable.so'),
                                       Path('/usr/lib64/darktable/libdarktable.so')]
                        if p.exists()), None)
    if not library:
        raise RuntimeError('Set DARKTABLE_LIBRARY to the installed libdarktable.so')
    dt = ctypes.CDLL(library, mode=ctypes.RTLD_GLOBAL)
    dt.dt_init.argtypes = [ctypes.c_int, ctypes.POINTER(ctypes.c_char_p),
                          ctypes.c_int, ctypes.c_int, ctypes.c_void_p]
    dt.dt_init.restype = ctypes.c_int
    dt.dt_cleanup.argtypes = []
    dt.dt_cleanup.restype = None
    argv = (ctypes.c_char_p * (len(arguments) + 1))(
        *[os.fsencode(arg) for arg in arguments], None)
    if dt.dt_init(len(arguments), argv, 0, 1, None):
        raise RuntimeError('darktable engine initialization failed')
    running = True

    def stop(*_):
        nonlocal running
        running = False

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    try:
        Path(os.environ['OMALUX_DEV_READY']).touch()
        while running:
            time.sleep(0.1)
    finally:
        dt.dt_cleanup()


def launch(split):
    qml = shutil.which('qml6') or shutil.which('qml')
    if not qml:
        raise RuntimeError('Install the Qt 6 QML runtime (qml6 or qml)')
    executable = os.environ.get('DARKTABLE_BIN', 'darktable')
    children = []
    with tempfile.TemporaryDirectory(prefix='omalux-dev-') as folder:
        session = Path(folder)
        config = session / 'config'
        config.mkdir()
        ready = session / 'ready'
        env = dict(os.environ, OMALUX_DEV_READY=str(ready))
        args = [executable, '--configdir', str(config), '--cachedir', str(session / 'cache'),
                '--library', str(session / 'library.db'), '--conf', 'write_sidecar_files=never',
                '--conf', 'show_splash_screen=false', '--conf', 'ui/show_welcome_screen=false']
        if split:
            # Signal readiness after Lua initialization without affecting normal preferences.
            (config / 'luarc').write_text('local dt=require "darktable"\n'
                'dt.control.dispatch(function() local f=assert(io.open(os.getenv("OMALUX_DEV_READY"),"w")); f:close() end)\n')
        else:
            args = [sys.executable, str(Path(__file__).resolve()), '--engine', *args]
        with (session / 'darktable.log').open('w+') as log:
            try:
                worker = subprocess.Popen(args, env=env, stdout=log, stderr=subprocess.STDOUT)
                children.append(worker)
                deadline = time.monotonic() + 45
                while not ready.exists():
                    if worker.poll() is not None or time.monotonic() > deadline:
                        log.flush(); log.seek(0)
                        raise RuntimeError('darktable failed to start:\n' + log.read()[-4000:])
                    time.sleep(0.1)
                print('darktable ready; starting Omalux.', flush=True)
                ui = subprocess.Popen([qml, str(Path(__file__).parent / 'ui/Main.qml')])
                children.append(ui)
                while ui.poll() is None and worker.poll() is None:
                    time.sleep(0.1)
                return ui.returncode if ui.returncode is not None else 1
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
        if sys.argv[1:2] == ['--engine']:
            engine(sys.argv[2:])
        else:
            if sys.argv[1:] not in ([], ['--split']):
                raise SystemExit('Usage: dev.py [--split]')
            sys.exit(launch('--split' in sys.argv))
    except KeyboardInterrupt:
        sys.exit(130)
    except Exception as error:
        sys.exit(str(error))
