#!/usr/bin/env python3
"""Read how darktable displays each slider and write it to omalux/design/display.json.

darktable already says in its module sources what a value means on screen: the unit it
carries, the factor between the stored number and the shown one, how many digits are
worth showing, and the range a slider covers before the user has to force it wider.
None of that is in the introspection data, so it is collected here from the calls that
build the module's own widgets.
"""
import argparse
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[3]

# g->widget = dt_bauhaus_slider_from_params(self, "field"). The field may be wrapped in N_(),
# and the call may sit inside another one, as when a colour picker is put around the slider.
BINDING = re.compile(
    r'(?P<widget>[A-Za-z_][\w.\->]*)\s*=\s*[^;]*?dt_bauhaus_slider_from_params\s*\(\s*[^,]+,\s*'
    r'(?:N_\(\s*)?"(?P<field>[^"]+)"', re.S)
# The calls that describe how the slider reads.
CALL = re.compile(
    r'dt_bauhaus_slider_set_(?P<what>format|digits|factor|offset|soft_range|soft_min|soft_max)'
    r'\s*\(\s*(?P<widget>[A-Za-z_][\w.\->\[\]]*)\s*,\s*(?P<args>[^;]*?)\)\s*;', re.S)

# Constants the sources use in these calls.
CONSTANTS = {'RAD_2_DEG': 57.29577951308232, 'M_PI': 3.141592653589793,
             'M_PI_F': 3.141592653589793, 'DT_IOP_ORDER_INFO': None}


def number(text):
    """A literal or a known constant, with the f/l suffixes and a leading sign."""
    text = text.strip().rstrip('f').rstrip('F')
    sign = 1.0
    while text[:1] in '+-':
        sign = -sign if text[0] == '-' else sign
        text = text[1:].strip()
    if text in CONSTANTS and CONSTANTS[text] is not None:
        return sign * CONSTANTS[text]
    try:
        return sign * float(text)
    except ValueError:
        return None


def split_arguments(text):
    """Split on commas that are not inside brackets."""
    parts, depth, current = [], 0, ''
    for character in text:
        if character in '([':
            depth += 1
        elif character in ')]':
            depth -= 1
        if character == ',' and depth == 0:
            parts.append(current)
            current = ''
        else:
            current += character
    parts.append(current)
    return [p.strip() for p in parts if p.strip()]


def collect(source):
    """Everything one module source says about the display of its sliders."""
    text = source.read_text(errors='replace')
    widgets = {match['widget']: match['field'] for match in BINDING.finditer(text)}
    if not widgets:
        return {}
    entries = {}
    for call in CALL.finditer(text):
        field = widgets.get(call['widget'])
        if not field:
            continue
        entry = entries.setdefault(field, {})
        what, arguments = call['what'], split_arguments(call['args'])
        if what == 'format':
            # The unit may be wrapped for translation, as _(" EV").
            literal = re.fullmatch(r'(?:N?_\(\s*)?"([^"]*)"\s*\)?', arguments[0] or '')
            if literal:
                entry['format'] = literal.group(1)
        elif what in ('digits', 'factor', 'offset'):
            value = number(arguments[0]) if arguments else None
            if value is not None:
                entry[what] = int(value) if what == 'digits' else value
        elif what == 'soft_range' and len(arguments) >= 2:
            low, high = number(arguments[0]), number(arguments[1])
            if low is not None and high is not None:
                entry['soft_minimum'], entry['soft_maximum'] = low, high
        elif what in ('soft_min', 'soft_max') and arguments:
            value = number(arguments[0])
            if value is not None:
                entry['soft_minimum' if what == 'soft_min' else 'soft_maximum'] = value
    return {field: entry for field, entry in entries.items() if entry}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--darktable', default=str(ROOT / 'darktable'))
    parser.add_argument('--output', default=str(ROOT / 'omalux/design/display.json'))
    arguments = parser.parse_args()
    sources = sorted((Path(arguments.darktable) / 'src/iop').glob('*.c'))
    if not sources:
        raise SystemExit(f'No module sources under {arguments.darktable}/src/iop')
    display, modules = {}, 0
    for source in sources:
        found = collect(source)
        if not found:
            continue
        modules += 1
        for field, entry in found.items():
            display[f'{source.stem}/{field}'] = entry
    destination = Path(arguments.output)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(display, indent=1, sort_keys=True) + '\n')
    counts = {key: sum(1 for e in display.values() if key in e)
              for key in ('format', 'digits', 'factor', 'soft_minimum')}
    print(f'{destination.relative_to(ROOT)}: {len(display)} sliders in {modules} modules')
    print('  ' + ', '.join(f'{name} {count}' for name, count in counts.items()))


if __name__ == '__main__':
    main()
