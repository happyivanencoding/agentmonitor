"""Capture only the Agent Monitor client area while its debug-only native harness runs.
Requires Pillow for development verification; never required by the installed desktop app.
"""
from __future__ import annotations
import ctypes
import ctypes.wintypes as w
import json
import os
from pathlib import Path
import re
import time
from PIL import ImageGrab

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / '.local' / 'ui'
REPORT = Path(os.environ['LOCALAPPDATA']) / 'AgentMonitor' / 'native-qa.json'
OUT.mkdir(parents=True, exist_ok=True)
user = ctypes.windll.user32
try:
    ctypes.windll.shcore.SetProcessDpiAwareness(2)
except OSError:
    pass

def capture(name: str) -> bool:
    expected = int((ROOT / '.local' / 'desktop-debug.pid').read_text().strip())
    windows: list[int] = []
    @ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
    def visit(hwnd: int, _: int) -> bool:
        pid = w.DWORD()
        user.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        if pid.value == expected and user.IsWindowVisible(hwnd):
            rect = w.RECT()
            user.GetClientRect(hwnd, ctypes.byref(rect))
            if rect.right > 700 and rect.bottom > 400:
                windows.append(hwnd)
        return True
    user.EnumWindows(visit, 0)
    if not windows:
        return False
    hwnd = windows[0]
    user.ShowWindow(hwnd, 9)
    user.SetWindowPos(hwnd, -1, 0, 0, 0, 0, 0x0003)
    user.SetForegroundWindow(hwnd)
    time.sleep(0.15)
    rect = w.RECT()
    point = w.POINT(0, 0)
    user.GetClientRect(hwnd, ctypes.byref(rect))
    user.ClientToScreen(hwnd, ctypes.byref(point))
    box = (point.x, point.y, point.x + rect.right, point.y + rect.bottom)
    ImageGrab.grab(bbox=box, all_screens=True).save(OUT / f'{name}.png')
    user.SetWindowPos(hwnd, -2, 0, 0, 0, 0, 0x0003)
    return True

seen: set[str] = set()
for _ in range(180):
    try:
        report = json.loads(REPORT.read_text('utf-8'))
        step = str(report.get('step', report.get('status', '')))
        if re.fullmatch(r'[a-z-]{1,40}', step) and step not in seen:
            if capture(step):
                seen.add(step)
                print('Captured native client:', step, flush=True)
        if report.get('status') in ('passed', 'failed'):
            report['capturedScreenshots'] = sorted(seen)
            (ROOT / '.local' / 'native-verification.json').write_text(json.dumps(report, indent=2), 'utf-8')
            print(json.dumps(report, ensure_ascii=False, indent=2), flush=True)
            raise SystemExit(0 if report['status'] == 'passed' else 1)
    except (FileNotFoundError, json.JSONDecodeError, ValueError):
        pass
    time.sleep(1)
raise SystemExit('Native QA did not finish within the bounded observation window.')
