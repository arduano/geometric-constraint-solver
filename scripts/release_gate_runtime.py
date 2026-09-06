#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Conservative native loader closure for execution beside mutable Cargo builds."""
from __future__ import annotations
import os
from pathlib import Path
import re
import shutil
import subprocess

SCHEMA = 'geosolve-native-runtime-v1'


def immutable(path):
    path = Path(path)
    # An external symlink into the store is still replaceable by Cargo or a
    # caller. Both the lookup path and its final target must belong to the store.
    return (path.is_absolute() and '..' not in path.parts and path.is_relative_to('/nix/store')
            and path.resolve().is_relative_to('/nix/store') and path.is_file())


def loader_environment(environment):
    return {name: value for name, value in environment.items()
            if name.startswith('LD_') or name == 'GLIBC_TUNABLES'}


def loader_lines(text):
    """Resolve only explicit successful glibc --list records."""
    result = []
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        if re.fullmatch(r'linux-vdso\.so\.\d+ \(0x[0-9a-f]+\)', line):
            continue
        match = re.fullmatch(r'(?:(\S+) => )?(/\S+) \(0x[0-9a-f]+\)', line)
        if not match:
            raise ValueError('unrecognized loader dependency record')
        result.append((match[1] or match[2], match[2]))
    if not result or len(result) != len(set(result)):
        raise ValueError('missing or duplicate loader dependency inventory')
    return sorted(result)


def inspect_runtime(executable, environment):
    """Return overlap eligibility; unknown/mutable dependencies keep the build lock."""
    from release_gate_native import file_hash
    original = dict(environment)
    readelf = shutil.which('readelf')
    try:
        readelf = str(Path(readelf).resolve()) if readelf else None
        if not readelf or not immutable(readelf):
            raise ValueError('no immutable readelf')
        if any(value for name, value in original.items() if name.startswith('LD_') and name != 'LD_LIBRARY_PATH'):
            raise ValueError('unsupported loader environment override')
        def capture(command, env):
            return subprocess.check_output(command, env=env, text=True, stderr=subprocess.PIPE, timeout=20)
        headers = capture([readelf, '-lW', str(executable)], original)
        interpreters = re.findall(r'\[Requesting program interpreter: (.+?)\]', headers)
        if len(interpreters) != 1 or not immutable(interpreters[0]):
            raise ValueError('unknown or mutable native interpreter')
        loader = interpreters[0]
        filtered = dict(original)
        filtered['LD_LIBRARY_PATH'] = ':'.join(value for value in original.get('LD_LIBRARY_PATH', '').split(':')
                                             if value and Path(value).is_absolute()
                                             and '..' not in Path(value).parts
                                             and Path(value).is_relative_to('/nix/store')
                                             and Path(value).resolve().is_relative_to('/nix/store'))
        before = loader_lines(capture([loader, '--list', str(executable)], original))
        after = loader_lines(capture([loader, '--list', str(executable)], filtered))
        if before != after or any(not immutable(path) for _, path in after):
            raise ValueError('native loader resolution depends on mutable build outputs')
        paths = {str(executable), loader, *(path for _, path in after)}
        for path in paths:
            dynamic = capture([readelf, '-dW', path], filtered)
            if re.search(r'\((?:AUDIT|DEPAUDIT|FILTER|AUXILIARY)\)', dynamic):
                raise ValueError('unsupported additional native loader directive')
            for raw in re.findall(r'\((?:RPATH|RUNPATH)\).*?\[(.*?)\]', dynamic):
                # glibc ignores an entirely empty dynamic search-path value;
                # empty components inside a nonempty list still mean cwd.
                if raw == '':
                    continue
                for entry in raw.split(':'):
                    entry = entry.replace('${ORIGIN}', str(Path(path).parent)).replace('$ORIGIN', str(Path(path).parent))
                    if (not entry or '$' in entry or not Path(entry).is_absolute()
                            or '..' in Path(entry).parts or not Path(entry).is_relative_to('/nix/store')
                            or not Path(entry).resolve().is_relative_to('/nix/store')):
                        raise ValueError('native runtime search path can select mutable libraries')
        closure = {path: file_hash(path) for path in sorted(paths - {str(executable)})}
        return {'schema': SCHEMA, 'safe': True, 'executable': str(executable),
                'loader': loader, 'readelf': str(Path(readelf).resolve()),
                'readelf_sha256': file_hash(readelf), 'libraries': closure,
                'ld_library_path': filtered['LD_LIBRARY_PATH'],
                'loader_environment': loader_environment(filtered)}
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        return {'safe': False, 'reason': str(error)}
