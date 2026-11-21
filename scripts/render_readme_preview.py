"""Generate the README illustration from built-in fixtures, never from a live AGY session."""
import os
from pathlib import Path
import re
import subprocess
import tempfile
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parents[1]
TERMINAL_STYLE = re.compile(r"\x1b\[([0-9;]*)m")


def colorize_line(line):
    runs = []
    color = "#cdd6f4"
    position = 0
    for match in TERMINAL_STYLE.finditer(line):
        if match.start() > position:
            runs.append(f'<tspan fill="{color}">{escape(line[position:match.start()])}</tspan>')
        codes = match.group(1).split(";")
        color = "#cdd6f4"
        for index in range(len(codes) - 4):
            if codes[index:index + 2] == ["38", "2"]:
                rgb = bytes(map(int, codes[index + 2:index + 5]))
                color = "#" + rgb.hex()
        position = match.end()
    if position < len(line):
        runs.append(f'<tspan fill="{color}">{escape(line[position:])}</tspan>')
    return "".join(runs)


def render_preview():
    executable = ROOT / "target/release" / ("agyline.exe" if os.name == "nt" else "agyline")
    blocks = []
    baseline = 74
    with tempfile.TemporaryDirectory(prefix="agyline-readme-") as temporary:
        sandbox = Path(temporary)
        environment = dict(os.environ, AGYLINE_HOME=str(sandbox / "tool"), AGY_CONFIG_DIR=str(sandbox / "host"))
        for scenario, heading in [("antigravity", "01   EVERYDAY"), ("agy-low-quota", "02   LOW QUOTA")]:
            result = subprocess.run([str(executable), "preview", "--scenario", scenario, "--width", "100", "--color", "truecolor"],
                                    cwd=sandbox, env=environment, capture_output=True, encoding="utf-8", check=True, timeout=10)
            blocks.append(f'<text x="28" y="{baseline}" fill="#a6adc8" font-size="13">{heading}</text>')
            for line in result.stdout.splitlines():
                baseline += 24
                blocks.append(f'<text x="28" y="{baseline}" xml:space="preserve">{colorize_line(line)}</text>')
            baseline += 40
    height = baseline - 12
    svg = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 900 {height}" role="img" aria-labelledby="title desc">',
           '<title id="title">AGYCometixLine | Compact Mocha</title>',
           '<desc id="desc">Actual renderer output for two built-in synthetic sessions. Healthy quotas are compact; low quotas show reset times.</desc>',
           f'<rect width="900" height="{height}" rx="12" fill="#1e1e2e"/>',
           '<g font-family="Consolas, DejaVu Sans Mono, monospace" font-size="16">',
           '<text x="28" y="32" fill="#b4befe" font-weight="bold">AGYCometixLine</text>',
           '<text x="872" y="32" fill="#a6adc8" font-size="12" text-anchor="end">COMPACT MOCHA / SAMPLE DATA</text>',
           '<path d="M28 48H872" stroke="#45475a"/>', *blocks, '</g></svg>']
    (ROOT / "assets/agyline-preview.svg").write_bytes(("\n".join(svg) + "\n").encode("utf-8"))


if __name__ == "__main__":
    render_preview()
