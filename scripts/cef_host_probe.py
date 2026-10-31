#!/usr/bin/env python3
"""Probe the shipped CEF panel with a synthetic WebSocket host.

The Remastered panels open ws://127.0.0.1:<port>/<route>/<guid> and exchange
JSON envelopes.  This script serves the extracted optionspanel, answers the
first requests, sends ShowTab, and captures the rendered panel.
"""
import asyncio
import http.server
import json
import socketserver
import threading
import time
from pathlib import Path

from playwright.sync_api import sync_playwright
from websockets.sync.server import serve

ROOT = Path("/tmp/opencode/webtest")
import re
_raw = json.load(open("/tmp/opencode/gluall.json"))
def _decode(v):
    v = v.replace("\\u0004", "\x04").replace("\\u0001", "\x01")
    if "\x04" in v:
        if len(v) > 1:
            v = v[1:]
        v = re.sub(r"\x04(.)\x01", r"\1", v)
    v = re.sub(r"[\x00-\x1f]", "", v)
    return v
STRINGS = {row["id"]: _decode(row.get("Value", "")) for row in _raw}
HTTP_PORT = 8267
WS_PORT = 8268
GUID = "11111111-2222-3333-4444-555555555555"


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=str(ROOT), **kw)

    def log_message(self, *a):
        pass


def ws_handler(conn):
    print("WS connected", conn.request.path)
    try:
        conn.send(json.dumps({
            "requestId": "host:1",
            "routes": ["allExceptMe"],
            "data": {"endpoint": "ShowTab", "data": {"name": "Speed", "settings": {
                "GameSpeed": {"value": 3}, "KeyScroll": {"value": 3},
                "MouseScroll": {"value": 3}, "MouseSensitivity": {"value": 3},
                "UseMouseSensitivity": {"value": True}, "MouseScaling": {"value": True},
                "HardwareCursor": {"value": False}, "UseMouseConfine": {"value": False}},
                "showMac": False}},
        }))
        for raw in conn:
            print("WS recv", raw[:500])
            try:
                msg = json.loads(raw)
            except Exception:
                continue
            endpoint = (msg.get("data") or {}).get("endpoint")
            rid = msg.get("requestId", "host")
            if endpoint == "GetStrings":
                ids = ((msg.get("data") or {}).get("data") or {}).get("ids") or []
                print("STRINGS", len(ids), ids[:8])
                strings = [{"id": i, "value": STRINGS.get(i, i)} for i in ids]
                conn.send(json.dumps({
                    "requestId": rid,
                    "routes": ["allExceptMe"],
                    "data": {"endpoint": "GetStrings.response",
                             "data": {"strings": strings}},
                }))
            else:
                print("IGNORED", endpoint)
    except Exception as e:
        print("WS closed", e)


def main():
    httpd = socketserver.TCPServer(("127.0.0.1", HTTP_PORT), Handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    server = serve(ws_handler, "127.0.0.1", WS_PORT)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    url = (f"http://127.0.0.1:{HTTP_PORT}/optionspanel/optionspanel.html"
           f"?port={WS_PORT}&guid={GUID}")
    with sync_playwright() as p:
        browser = p.chromium.launch()
        page = browser.new_page(viewport={"width": 1920, "height": 1200})
        page.on("console", lambda m: print("CONSOLE", m.type, m.text))
        page.on("pageerror", lambda e: print("PAGEERROR", e, getattr(e, "stack", "")))
        page.goto(url)
        page.wait_for_timeout(4000)
        page.add_style_tag(content=".Tab-label>span[item]>span:not(:first-child){display:none!important}")
        print("ROOT_HTML", page.eval_on_selector("#root", "e => e.innerHTML")[:3000])
        print("BODY_TEXT", page.inner_text("body")[:1000])
        print("PANEL", page.evaluate("() => document.querySelector('.OptionsPanel') ? document.querySelector('.OptionsPanel').outerHTML.slice(0,1000) : 'none'"))
        page.screenshot(path="/tmp/opencode/cef-options-1920.png")
        browser.close()
    server.shutdown()
    httpd.shutdown()


if __name__ == "__main__":
    main()
