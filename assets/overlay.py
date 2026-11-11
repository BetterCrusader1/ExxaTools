#!/usr/bin/env python3
"""Frameless always-on-top panel for the Steam menu.

Steam draws its store, library and Community windows above the main frame, so a
panel inside the client's DOM is covered. This is a separate window instead: it
floats above Steam and closes when it loses focus.

Usage: overlay.py --port 8765 --x 1200 --y 60 --width 300 --height 384
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

# This machine has no usable GBM allocator for WebKit's compositor.
os.environ.setdefault("WEBKIT_DISABLE_COMPOSITING_MODE", "1")
os.environ.setdefault("WEBKIT_DISABLE_DMABUF_RENDERER", "1")

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
gi.require_version("WebKit2", "4.1")
from gi.repository import Gdk, Gtk, WebKit2  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--x", type=int, default=200)
    parser.add_argument("--y", type=int, default=60)
    parser.add_argument("--width", type=int, default=300)
    parser.add_argument("--height", type=int, default=384)
    args = parser.parse_args()

    page = Path(__file__).resolve().parent / "overlay.html"
    if not page.is_file():
        print(f"overlay page missing: {page}", file=sys.stderr)
        return 1

    window = Gtk.Window(type=Gtk.WindowType.TOPLEVEL)
    window.set_decorated(False)
    window.set_resizable(False)
    window.set_skip_taskbar_hint(True)
    window.set_skip_pager_hint(True)
    window.set_keep_above(True)
    window.set_type_hint(Gdk.WindowTypeHint.UTILITY)
    window.set_focus_on_map(True)
    window.set_modal(False)
    window.set_accept_focus(True)
    window.set_default_size(args.width, args.height)
    window.move(args.x, args.y)

    view = WebKit2.WebView()
    settings = view.get_settings()
    settings.set_property("enable-write-console-messages-to-stdout", False)
    try:
        settings.set_property("hardware-acceleration-policy", WebKit2.HardwareAccelerationPolicy.NEVER)
    except Exception:
        pass
    settings.set_property("allow-file-access-from-file-urls", True)
    settings.set_property("allow-universal-access-from-file-urls", True)
    view.set_background_color(Gdk.RGBA(0.082, 0.102, 0.129, 1))

    url = f"file://{page}?port={args.port}"
    view.connect("load-failed", lambda _view, _event, uri, error: print(f"load failed {uri}: {error}", file=sys.stderr))
    view.connect("load-changed", lambda _view, event: print(f"load {event.value_nick}", file=sys.stderr))
    view.connect("web-process-terminated", lambda _view, reason: print(f"web process gone: {reason.value_nick}", file=sys.stderr))
    view.load_uri(url)
    window.add(view)

    def close(*_args) -> bool:
        Gtk.main_quit()
        return False

    window.connect("destroy", close)
    window.connect("delete-event", close)
    window.show_all()
    window.present()
    Gtk.main()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())