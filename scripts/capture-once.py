"""Development helper: capture only the client rectangle owned by a specified Monitor PID."""
import ctypes
import ctypes.wintypes as w
import sys
import time
from pathlib import Path
from PIL import ImageGrab

pid = int(sys.argv[1])
output = Path(sys.argv[2])
u = ctypes.windll.user32
try:
    ctypes.windll.shcore.SetProcessDpiAwareness(2)
except OSError:
    pass
matches = []
@ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
def visit(hwnd, _):
    owner = w.DWORD()
    u.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
    rect = w.RECT()
    u.GetClientRect(hwnd, ctypes.byref(rect))
    if owner.value == pid and u.IsWindowVisible(hwnd) and rect.right > 700 and rect.bottom > 400:
        matches.append(hwnd)
    return True
u.EnumWindows(visit, 0)
if not matches:
    raise SystemExit(f'No visible Monitor window for PID {pid}')
hwnd = matches[0]
u.ShowWindow(hwnd, 9)
u.SetWindowPos(hwnd, -1, 0, 0, 0, 0, 0x0003)
u.SetForegroundWindow(hwnd)
time.sleep(0.5)
rect = w.RECT()
point = w.POINT()
u.GetClientRect(hwnd, ctypes.byref(rect))
u.ClientToScreen(hwnd, ctypes.byref(point))
output.parent.mkdir(parents=True, exist_ok=True)
ImageGrab.grab(bbox=(point.x, point.y, point.x + rect.right, point.y + rect.bottom), all_screens=True).save(output)
u.SetWindowPos(hwnd, -2, 0, 0, 0, 0, 0x0003)
print(f'Captured {rect.right} x {rect.bottom} native client to {output}')
