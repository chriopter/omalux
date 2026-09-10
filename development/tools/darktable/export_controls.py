#!/usr/bin/env python3
"""Write the curated control registry to omalux/design/controls.json.

The registry in omalux/native/engine/controls.h holds two different kinds of thing. Most
of each row repeats what darktable already says about the parameter: its range, its unit,
how many digits it shows. That part is now read from darktable itself. What is left is
our own decision: which parameters are worth showing, under which name, in which group
and order, and with which colour track.

This tool separates the two. It writes the decisions as data, and reports which rows carry
values that disagree with what darktable says, so a row is only kept where we mean to
differ or where darktable draws the widget itself and says nothing.
"""
import argparse
import json
import math
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[3]
ROW = re.compile(r'\{"(?P<id>[^"]+)", "(?P<label>[^"]*)", "(?P<module>[^"]+)", '
                 r'"(?P<parameter>[^"]+)", (?P<rest>[^}]+)\}')
FIELDS = ['minimum', 'maximum', 'step', 'initial', 'scale', 'offset', 'unit', 'decimals',
          'group', 'section', 'action', 'colors', 'detail', 'soft_minimum', 'soft_maximum']


def parse(text):
    """The registry rows, as dictionaries."""
    rows = []
    for match in ROW.finditer(text):
        values = [v.strip() for v in match['rest'].split(',')]
        row = {'id': match['id'], 'label': match['label'], 'module': match['module'],
               'parameter': match['parameter']}
        for name, raw in zip(FIELDS, values):
            if raw.startswith('"'):
                text = raw.strip('"')
                # Only escaped sequences need unescaping; the file itself is UTF-8.
                row[name] = (text.encode('latin-1', 'backslashreplace').decode('unicode_escape')
                             if '\\' in text else text)
            else:
                row[name] = float(raw) if '.' in raw or 'e' in raw else int(raw)
        rows.append(row)
    return rows


def disagreements(row, display):
    """Where our row says something other than darktable does."""
    entry = display.get(f"{row['module']}/{row['parameter']}")
    if entry is None:
        return ['darktable draws this widget itself']
    found = []
    scale = row.get('scale', 1) or 1
    # Our digits are counted on the displayed value, so a power-of-ten scale shifts them.
    # Any other factor (ISO coarseness, say) is not comparable and the digits are ours.
    shift = round(math.log10(1 / abs(scale)))
    if abs(abs(scale) - 10.0 ** -shift) > 1e-9:
        return []
    if 'format' in entry and entry['format'].strip() != row.get('unit', '').strip():
        found.append(f"unit {row.get('unit','')!r} against {entry['format']!r}")
    if 'digits' in entry and entry['digits'] - shift != row.get('decimals'):
        found.append(f"digits {row.get('decimals')} against {entry['digits'] - shift}")
    if 'soft_minimum' in entry and 'soft_minimum' not in row:
        found.append('no soft range where darktable suggests one')
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--registry', default=str(ROOT / 'omalux/native/engine/controls.h'))
    parser.add_argument('--display', default=str(ROOT / 'omalux/design/display.json'))
    parser.add_argument('--output', default=str(ROOT / 'omalux/design/controls.json'))
    arguments = parser.parse_args()
    rows = parse(Path(arguments.registry).read_text())
    display = json.loads(Path(arguments.display).read_text())

    controls, groups, special = [], [], 0
    for order, row in enumerate(rows):
        if row['group'] not in groups:
            groups.append(row['group'])
        entry = {'id': row['id'], 'module': row['module'], 'parameter': row['parameter'],
                 'label': row['label'], 'group': row['group'], 'section': row['section'],
                 'order': order, 'detail': bool(row['detail'])}
        if row['colors']:
            entry['colors'] = row['colors']
        # A parameter darktable does not describe needs its own adapter; say so in the data.
        if row['parameter'].startswith('@') or f"{row['module']}/{row['parameter']}" not in display:
            entry['adapter'] = True
            special += 1
        controls.append(entry)

    destination = Path(arguments.output)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps({'groups': groups, 'controls': controls}, indent=1) + '\n')
    print(f'{destination.relative_to(ROOT)}: {len(controls)} controls in {len(groups)} groups, '
          f'{special} needing their own adapter')
    reported = [(row['id'], found) for row in rows
                if not row['parameter'].startswith('@') and (found := disagreements(row, display))]
    if reported:
        print(f'\n{len(reported)} rows say something other than darktable does:')
        for name, found in reported:
            print(f'  {name:22} {"; ".join(found)}')


if __name__ == '__main__':
    main()
