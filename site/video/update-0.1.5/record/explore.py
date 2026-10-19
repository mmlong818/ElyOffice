# Usage: explore.py <app> <exe> <file|-> <out.png> [python actions]
import sys
import time
from pathlib import Path

from drive import *  # noqa: F403

app, exe, file, out = sys.argv[1:5]
actions = sys.argv[5] if len(sys.argv) > 5 else ""
with fresh_data():
    files = [] if file == "-" else [Path(file).resolve()]
    proc, h = launch(app, exe, *files)
    exec(actions)
    time.sleep(0.6)
    shot(out)
