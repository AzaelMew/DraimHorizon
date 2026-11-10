#!/usr/bin/env python3
"""Regenerate bundled Lucide geometry: uv run --with svgpathtools tools/update_lucide.py.

The generator is development-only; the mod has no SVG/runtime dependency.
"""
from concurrent.futures import ThreadPoolExecutor
import math
from pathlib import Path
from urllib.request import urlopen

from svgpathtools import Line, svgstr2paths

ROOT = Path(__file__).resolve().parents[1]
REVISION = "53fab88ff9d1b48f4704b5b5b560d6b4d560bf4c"
BASE = f"https://raw.githubusercontent.com/lucide-icons/lucide/{REVISION}"
ICONS = dict(Warning="triangle-exclamation-point", Search="search", Portal="door-open", Chest="archive", Star="star",
             Pin="map-pin", People="users", Map="map", Chat="message-square",
             Market="store", Close="x", Settings="settings", Plus="plus",
             Minus="minus", Back="chevron-left", Next="chevron-right", Check="check",
             Trash="trash", Refresh="refresh-cw", Sort="arrow-down-up",
             Fill="list-start", Restock="package-plus", Transfer="arrow-right-left",
             Wallet="wallet", Clock="clock", Signal="wifi", Eye="eye", Home="house",
             Grid="grid-2x2", Layers="layers", Chart="chart-no-axes-combined",
             Info="info", Shield="shield", Save="save", Gauge="gauge", Activity="activity",
             Coins="coins", Skull="skull", Locate="locate-fixed")


def download(path):
    with urlopen(f"{BASE}/{path}", timeout=30) as response:
        return response.read().decode("utf-8")


def geometry(entry):
    key, name = entry
    try:
        paths, _ = svgstr2paths(download(f"icons/{name}.svg"))
    except Exception as error:
        raise RuntimeError(name) from error
    lines = []
    for path in paths:
        for segment in path:
            steps = 1 if isinstance(segment, Line) else max(2, math.ceil(segment.length() / 1.0))
            for i in range(steps):
                a, b = segment.point(i / steps) / 2, segment.point((i + 1) / steps) / 2
                if abs(b - a) > 0.0001:
                    lines.append(f"            [{a.real:.4f}, {a.imag:.4f}, {b.real:.4f}, {b.imag:.4f}],")
    return f"        {key} => &[\n" + "\n".join(lines) + "\n        ],"


def main():
    with ThreadPoolExecutor(max_workers=8) as pool:
        entries = list(pool.map(geometry, ICONS.items()))
    target = ROOT / "mixins/draimhorizon/ui_style.rs"
    source = target.read_text(encoding="utf-8")
    start = source.index("/// Original outline icons") if "/// Original outline icons" in source else source.index("/// Lucide icons")
    # Replace only the generated function; helpers may follow it before icon_for.
    end = source.index("\n}\n", source.index("pub fn strokes", start)) + 3
    generated = f"/// Lucide icons ({REVISION}), ISC license in notices/LICENSE-lucide.\n"
    generated += "/// Official 24px SVG curves flattened onto the native 12px stroke grid.\n"
    generated += '#[allow(clippy::approx_constant, reason = "flattened official SVG coordinates")]\n'
    generated += "pub fn strokes(icon: Icon) -> &'static [[f32; 4]] {\n    use Icon::*;\n    match icon {\n"
    generated += "\n".join(entries) + "\n    }\n}\n"
    target.write_text(source[:start] + generated + source[end:], encoding="utf-8")
    license_text = download("LICENSE")
    (ROOT / "notices/LICENSE-lucide").write_text(license_text, encoding="utf-8")
    # Ship attribution within the native archive too (Rust/TOML source format).
    (ROOT / "mixins/draimhorizon/lucide_license.rs").write_text("/*\n" + license_text + "\n*/\n", encoding="utf-8")


if __name__ == "__main__":
    main()

