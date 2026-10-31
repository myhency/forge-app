#!/usr/bin/env python3
"""Adds sentences to crates/forge-shared/src/ko.rs at their sorted place.

usage: python3 scripts/i18n_add.py 'English' 'Korean' ['English2' 'Korean2' ...]
Run it from the repository root. A sentence that is already there stops the script.
"""
import json
import re
import sys

PATH = 'crates/forge-shared/src/ko.rs'
LINE = re.compile(r'^    \("((?:[^"\\]|\\.)*)", "((?:[^"\\]|\\.)*)"\),$')


def main(arguments):
    if len(arguments) % 2 != 0 or not arguments:
        sys.exit(__doc__)
    lines = open(PATH, encoding='utf-8').read().split('\n')
    for english, korean in zip(arguments[0::2], arguments[1::2]):
        key = english.encode('utf-8')
        at = None
        for index, line in enumerate(lines):
            match = LINE.match(line)
            if not match:
                continue
            existing = json.loads('"' + match.group(1) + '"').encode('utf-8')
            if existing == key:
                sys.exit('already there: ' + english)
            if existing > key:
                at = index
                break
        if at is None:
            at = lines.index('];')
        lines.insert(at, '    ("%s", "%s"),' % (english, korean))
    open(PATH, 'w', encoding='utf-8').write('\n'.join(lines))


main(sys.argv[1:])
