"""Copies synced takes into public/footage: prep.py <takes dir>"""
import json
import shutil
import sys
from pathlib import Path

TAKES = ["menus", "cells", "doc", "table", "show", "more"]
# Recorded submenu panels (x0, y0, x1, y1) for the triangle.
SUBMENUS = {
    "menus": {"parent": [1100, 610, 1396, 652], "panel": [1394, 576, 1784, 1144]},
    "doc": {"parent": [1072, 702, 1370, 742], "panel": [1364, 698, 1660, 874]},
    "show": {"parent": [1182, 598, 1478, 638], "panel": [1474, 594, 1770, 774]},
}

root = Path(sys.argv[1])
out = Path(__file__).resolve().parent.parent / "public" / "footage"
out.mkdir(parents=True, exist_ok=True)
for name in TAKES:
    offset = json.loads((root / f"{name}.sync.json").read_text())["offset"]
    events = json.loads((root / f"{name}.json").read_text(encoding="utf-8"))
    for event in events:
        event["t"] = round(event["t"] + offset, 4)
    shutil.copyfile(root / f"{name}.60.mp4", out / f"{name}.mp4")
    data = {"fps": 60, "size": [2048, 1152], "events": events, "menus": SUBMENUS.get(name)}
    (out / f"{name}.json").write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")
    actions = [e for e in events if e["kind"] not in ("move", "sync")]
    print(name, " ".join(f"{e['kind']}@{e['t']:.2f}" for e in actions))
