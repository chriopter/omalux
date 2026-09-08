"""Resolve declared bundle assets into a private darktable configuration."""
import json
import shutil
from pathlib import Path, PurePosixPath


def bundle_file(bundle, name):
    if not isinstance(name, str) or not name or '\\' in name:
        raise ValueError('Asset path must be a relative filename')
    path = PurePosixPath(name)
    if path.is_absolute() or '..' in path.parts:
        raise ValueError(f'Asset path escapes its bundle: {name}')
    result = (bundle / name).resolve()
    if not result.is_relative_to(bundle.resolve()) or not result.is_file():
        raise ValueError(f'Missing bundle file: {name}')
    return result


def read_manifest(bundle):
    path = bundle / 'preset.json'
    if not path.exists():
        if (bundle / 'reference.json').exists():
            raise ValueError('Convert reference.json to preset.json before loading this bundle')
        return {'version': 1}
    manifest = json.loads(path.read_text())
    if not isinstance(manifest, dict) or type(manifest.get('version')) is not int or manifest.get('version') != 1:
        raise ValueError('Unsupported manifest format')
    assets = manifest.get('assets', [])
    if not isinstance(assets, list):
        raise ValueError('assets must be a list')
    for asset in assets:
        if not isinstance(asset, dict):
            raise ValueError('Asset must be an object')
        bundle_file(bundle, asset.get('path'))
    preview = manifest.get('preview', {})
    if not isinstance(preview, dict):
        raise ValueError('preview must be an object')
    for field in ('source', 'darktable_version'):
        if field in preview and not isinstance(preview[field], str):
            raise ValueError(f'preview.{field} must be a string')
    return manifest


def prepare_assets(catalogue, configs):
    """Return per-style errors; validate all destinations before copying any."""
    errors, plans, owners = {}, {}, {}
    destinations = {'watermark': 'watermarks', 'icc-input': 'color/in', 'icc-output': 'color/out'}
    for style in sorted(catalogue.rglob('preset.dtstyle')):
        key = style.relative_to(catalogue).as_posix()
        try:
            manifest = read_manifest(style.parent)
            plan = []
            for asset in manifest.get('assets', []):
                source = bundle_file(style.parent, asset['path'])
                role = asset.get('role')
                if role == 'lut':
                    # Styles store paths relative to the catalogue/LUT root.
                    continue
                if not isinstance(role, str) or role not in destinations:
                    raise ValueError(f'Unsupported asset role: {role}; requires engine integration')
                target = asset.get('target', source.name)
                if not isinstance(target, str) or not target or target in ('.', '..') or '/' in target or '\\' in target:
                    raise ValueError('Asset target must be a filename')
                destination = destinations[role] + '/' + target
                plan.append((source, destination))
            plans[key] = plan
            for source, destination in plan:
                owners.setdefault(destination, []).append((key, source))
        except (ValueError, OSError) as error:
            errors[key] = str(error)
    for destination, entries in owners.items():
        if len({source.read_bytes() for _, source in entries}) > 1:
            for key, _ in entries:
                errors[key] = f'Conflicting asset destination: {destination}'
    for key, plan in plans.items():
        if key not in errors:
            for source, destination in plan:
                for config in configs:
                    target = config / destination
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(source, target)
    return errors
