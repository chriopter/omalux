#!/usr/bin/env python3
"""Read how darktable displays each slider and write it to omalux/design/display.json.

darktable already says in its module sources what a value means on screen: the unit it
carries, the factor between the stored number and the shown one, how many digits are
worth showing, and the range a slider covers before the user has to force it wider.
None of that is in the introspection data, so it is collected here from the calls that
build the module's own widgets.

The result is what darktable actually shows, not what the calls literally say: the calls
are replayed in source order with Bauhaus' own rules, starting from the digits that
dt_bauhaus_slider_from_params derives from the parameter's hard range. Those ranges are
read from the installed darktable plugins, so run this against the release the adapter
is built for.
"""
import argparse
import ctypes
import json
import math
import os
from pathlib import Path
import re
import struct

ROOT = Path(__file__).resolve().parents[3]

# g->widget = dt_bauhaus_slider_from_params(self, "field"). The field may be wrapped in N_(),
# and the call may sit inside another one, as when a colour picker is put around the slider.
BINDING = re.compile(
    r'(?P<widget>[A-Za-z_][\w.\->]*)\s*=\s*[^;]*?dt_bauhaus_slider_from_params\s*\(\s*[^,]+,\s*'
    r'(?:N_\(\s*)?"(?P<field>[^"]+)"', re.S)
# The calls that describe how the slider reads.
CALL = re.compile(
    r'dt_bauhaus_slider_set_(?P<what>format|digits|factor|offset|soft_range|soft_min|soft_max|'
    r'hard_min|hard_max)\s*\(\s*(?P<widget>[A-Za-z_][\w.\->\[\]]*)\s*,\s*(?P<args>[^;]*?)\)\s*;', re.S)

# Constants the sources use in these calls.
CONSTANTS = {'RAD_2_DEG': 57.29577951308232, 'M_PI': 3.141592653589793,
             'M_PI_F': 3.141592653589793, 'DT_IOP_ORDER_INFO': None}

# common/introspection.h, DT_INTROSPECTION_VERSION 8.
TYPE_FLOAT, TYPE_USHORT, TYPE_INT = 2, 9, 10


class Header(ctypes.Structure):
    _fields_ = [('type', ctypes.c_int), ('type_name', ctypes.c_char_p), ('name', ctypes.c_char_p),
                ('field_name', ctypes.c_char_p), ('description', ctypes.c_char_p),
                ('size', ctypes.c_size_t), ('offset', ctypes.c_size_t), ('so', ctypes.c_void_p)]


def limits(kind):
    """The Min/Max/Default triple that follows the header of one scalar type."""
    class Field(ctypes.Structure):
        _fields_ = [('header', Header), ('Min', kind), ('Max', kind), ('Default', kind)]
    return Field


SCALARS = {TYPE_FLOAT: limits(ctypes.c_float), TYPE_INT: limits(ctypes.c_int),
           TYPE_USHORT: limits(ctypes.c_ushort)}


class Introspection:
    """The hard ranges darktable compiled into each module, read from the installed plugins.

    dt_bauhaus_slider_from_params derives a slider's digits from them, and the percent rule
    of dt_bauhaus_slider_set_format depends on the hard maximum, so the extractor needs the
    numbers darktable uses at run time."""

    def __init__(self, library):
        self.library = Path(library).resolve()
        core = ctypes.CDLL(str(self.library), mode=ctypes.RTLD_GLOBAL)
        self.version = ctypes.string_at(ctypes.addressof(
            ctypes.c_char.in_dll(core, 'darktable_package_version'))).decode()
        self.plugins = self.library.parent / 'plugins'
        self.modules = {}

    def available(self, operation):
        return (self.plugins / f'lib{operation}.so').is_file()

    def field(self, operation, name):
        """(type, minimum, maximum) of a slider parameter, or None if darktable does not describe it."""
        if operation not in self.modules:
            try:
                module = ctypes.CDLL(str(self.plugins / f'lib{operation}.so'))
                module.get_f.restype = ctypes.c_void_p
                module.get_f.argtypes = [ctypes.c_char_p]
            except (OSError, AttributeError):
                module = None
            self.modules[operation] = module
        module = self.modules[operation]
        # "field[2]" is an element of an array; darktable looks up "field[0]".
        indexed = re.fullmatch(r'([^\[]+)\[\d+\]', name)
        address = module.get_f((indexed.group(1) + '[0]' if indexed else name).encode()) if module else None
        if not address:
            return None
        kind = Header.from_address(address).type
        if kind not in SCALARS:
            return None
        field = SCALARS[kind].from_address(address)
        return kind, float(field.Min), float(field.Max)


def f32(value):
    """Round to single precision, as darktable's float arithmetic does."""
    return struct.unpack('f', struct.pack('f', value))[0]


def initial_state(described):
    """What dt_bauhaus_slider_from_params (develop/imageop_gui.c) starts with: digits from the
    hard range for floats, none for integers; factor 1, offset 0 and no unit."""
    state = {'factor': 1.0, 'offset': 0.0, 'format': None, 'digits': None,
             'hard_minimum': None, 'hard_maximum': None}
    if described:
        kind, low, high = described
        state['hard_minimum'], state['hard_maximum'] = low, high
        if kind == TYPE_FLOAT:
            top = min(f32(high - low), max(abs(low), abs(high)))
            state['digits'] = max(2, -math.floor(math.log10(top / 100) + .1)) if 0 < top < math.inf else 2
        else:
            state['digits'] = 0
    return state


# A numeric #define in the module source, such as GRAIN_SCALE_FACTOR.
DEFINE = re.compile(r'^\s*#\s*define\s+(\w+)\s+\(?\s*(-?[\d.]+(?:[eE][-+]?\d+)?)[fF]?\s*\)?\s*(?:/[/*].*)?$',
                    re.M)


def number(text, defines=None):
    """A literal or a known constant, with the f/l suffixes and a leading sign."""
    text = text.strip()
    if re.fullmatch(r'[-+\s]*[\d.]+(?:[eE][-+]?\d+)?[fFlL]?', text):
        text = text.rstrip('fFlL')
    sign = 1.0
    while text[:1] in '+-':
        sign = -sign if text[0] == '-' else sign
        text = text[1:].strip()
    if defines and text in defines:
        return sign * defines[text]
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


def apply_call(state, what, arguments, defines):
    """One dt_bauhaus_slider_set_* call, with the effect bauhaus/bauhaus.c gives it."""
    value = number(arguments[0], defines) if arguments else None
    if what == 'format':
        # The unit may be wrapped for translation, as _(" EV").
        literal = re.fullmatch(r'(?:N?_\(\s*)?"([^"]*)"\s*\)?', arguments[0] if arguments else '')
        if not literal:
            return True
        state['format'] = literal.group(1)
        # dt_bauhaus_slider_set_format: a percent unit on a slider whose hard maximum is at
        # most 10 shows the value times 100 (unless a factor is already set) with two
        # digits fewer. Digits set before the unit are therefore reduced, later ones not.
        if '%' in state['format']:
            if state['hard_maximum'] is None or state['digits'] is None:
                return False
            if abs(state['hard_maximum']) <= 10:
                if state['factor'] == 1.0:
                    state['factor'] = 100.0
                state['digits'] -= 2
    elif what == 'digits' and value is not None:
        state['digits'] = int(value)
    elif what in ('factor', 'offset') and value is not None:
        state[what] = value
    elif what in ('hard_min', 'hard_max') and value is not None:
        # The slider's own limit, narrower or wider than the parameter's; a soft limit
        # beyond it moves with it (dt_bauhaus_slider_set_hard_min/max).
        name = 'hard_minimum' if what == 'hard_min' else 'hard_maximum'
        state[name] = value
        state['hard_changed'] = True
        soft = 'soft_' + name[5:]
        if soft in state:
            state[soft] = max(state[soft], value) if what == 'hard_min' else min(state[soft], value)
    elif what == 'soft_range' and len(arguments) >= 2:
        low, high = number(arguments[0], defines), number(arguments[1], defines)
        if low is not None and high is not None:
            set_soft(state, 'soft_minimum', low)
            set_soft(state, 'soft_maximum', high)
    elif what in ('soft_min', 'soft_max') and value is not None:
        set_soft(state, 'soft_minimum' if what == 'soft_min' else 'soft_maximum', value)
    return True


def set_soft(state, name, value):
    """darktable clamps a soft limit into the hard range current at the time of the call."""
    if state['hard_minimum'] is not None:
        value = min(max(value, state['hard_minimum']), state['hard_maximum'])
    state[name] = value


def module_sources(darktable):
    """(operation, source) for every processing module, as src/iop/CMakeLists.txt builds them.
    The operation is the plugin name; it differs from the file name for a few modules
    (colorreconstruct is built from colorreconstruction.c, lens from lens.cc)."""
    folder = Path(darktable) / 'src/iop'
    listing = (folder / 'CMakeLists.txt').read_text()
    return sorted((name, folder / source)
                  for name, source in re.findall(r'add_iop\(\s*(\w+)\s+"([^"]+)"', listing))


def collect(operation, source, introspection):
    """Everything one module source says about the display of its sliders, and how many
    percent units could not be resolved because the parameter's range is unknown."""
    text = source.read_text(errors='replace')
    events = [(match.start('field'), 'bind', match) for match in BINDING.finditer(text)]
    if not events:
        return {}, 0
    events += [(match.start(), 'call', match) for match in CALL.finditer(text)]
    defines = {name: float(value) for name, value in DEFINE.findall(text)}
    events.sort(key=lambda event: event[0])
    # Replay in source order: a widget variable may be reused for several sliders, and one
    # field may get several sliders (bilat shows sigma_r as "range" or "highlights").
    widgets, sliders, unresolved = {}, [], 0
    for _, kind, match in events:
        if kind == 'bind':
            state = initial_state(introspection.field(operation, match['field']))
            widgets[match['widget']] = state
            sliders.append((match['field'], state))
        elif match['widget'] in widgets:
            if not apply_call(widgets[match['widget']], match['what'], split_arguments(match['args']), defines):
                unresolved += 1
    entries = {}
    for field, state in sliders:
        entry = describe(state)
        if not entry:
            continue
        # The first slider of a field is its entry; others are listed as alternatives.
        if field in entries:
            entries[field].setdefault('alternatives', []).append(entry)
        else:
            entries[field] = entry
    return entries, unresolved


def describe(state):
    """The entry written for one slider."""
    entry = {}
    if state['format'] is not None:
        entry['format'] = state['format']
    if state['digits'] is not None:
        entry['digits'] = state['digits']
    if state['factor'] != 1.0:
        entry['factor'] = state['factor']
    if state['offset'] != 0.0:
        entry['offset'] = state['offset']
    for name in ('soft_minimum', 'soft_maximum'):
        if name in state:
            entry[name] = state[name]
    if state.get('hard_changed'):
        entry['hard_minimum'], entry['hard_maximum'] = state['hard_minimum'], state['hard_maximum']
    return entry


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--darktable', default=str(ROOT / 'darktable'))
    parser.add_argument('--library', default=os.environ.get('DARKTABLE_LIBRARY',
                                                            '/usr/lib/darktable/libdarktable.so'),
                        help='installed libdarktable.so; its plugins supply the hard ranges')
    parser.add_argument('--output', default=str(ROOT / 'omalux/design/display.json'))
    arguments = parser.parse_args()
    sources = module_sources(arguments.darktable)
    if not sources:
        raise SystemExit(f'No module sources under {arguments.darktable}/src/iop')
    introspection = Introspection(arguments.library)
    display, modules, unresolved = {}, 0, 0
    # Modules the installed release does not ship (such as the "useless" example) are skipped.
    skipped = [operation for operation, _ in sources if not introspection.available(operation)]
    for operation, source in sources:
        if operation in skipped:
            continue
        found, missing = collect(operation, source, introspection)
        unresolved += missing
        if not found:
            continue
        modules += 1
        for field, entry in found.items():
            display[f'{operation}/{field}'] = entry
    destination = Path(arguments.output)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(display, indent=1, sort_keys=True) + '\n')
    counts = {key: sum(1 for e in display.values() if key in e)
              for key in ('format', 'digits', 'factor', 'soft_minimum')}
    print(f'{destination.relative_to(ROOT)}: {len(display)} sliders in {modules} modules '
          f'(hard ranges from darktable {introspection.version})')
    print('  ' + ', '.join(f'{name} {count}' for name, count in counts.items()))
    if skipped:
        print('  not installed, skipped: ' + ', '.join(skipped))
    if unresolved:
        print(f'  {unresolved} percent units on sliders without a described range were left as written')


if __name__ == '__main__':
    main()
