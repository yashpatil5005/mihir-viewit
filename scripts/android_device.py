from __future__ import annotations

import base64
import hashlib
import json
import os
import socket
import struct
import subprocess
import time
import urllib.request
from pathlib import Path
from typing import Any


DEFAULT_PACKAGE = "ai.viewit.app"
DEFAULT_ACTIVITY = "ai.viewit.app/.MainActivity"


def run(args: list[str], *, check: bool = True, capture: bool = True, binary: bool = False):
    result = subprocess.run(
        args,
        check=check,
        text=not binary,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
    )
    return result.stdout if capture else (b"" if binary else "")


def adb_command(serial: str | None = None) -> list[str]:
    command = [os.environ.get("ADB_PATH", "adb")]
    if serial:
        command += ["-s", serial]
    return command


def _read_exact(sock: socket.socket, length: int) -> bytes:
    data = bytearray()
    while len(data) < length:
        chunk = sock.recv(length - len(data))
        if not chunk:
            raise RuntimeError("WebSocket closed mid-frame")
        data.extend(chunk)
    return bytes(data)


class CdpClient:
    def __init__(self, websocket_url: str, timeout: int = 15):
        self.websocket_url = websocket_url
        self.timeout = timeout
        self._next_id = 0
        self._socket: socket.socket | None = None

    def __enter__(self):
        parts = self.websocket_url.split("/")
        hostport = parts[2]
        host, port = hostport.split(":")
        path = "/" + "/".join(parts[3:])
        sock = socket.create_connection((host, int(port)), timeout=5)
        key = base64.b64encode(os.urandom(16)).decode()
        request = (
            f"GET {path} HTTP/1.1\r\n"
            f"Host: {hostport}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\n"
            "Sec-WebSocket-Version: 13\r\n\r\n"
        )
        sock.sendall(request.encode())
        response = bytearray()
        while not response.endswith(b"\r\n\r\n"):
            chunk = sock.recv(1)
            if not chunk:
                raise RuntimeError("WebSocket handshake closed early")
            response.extend(chunk)
        if b"101" not in response.split(b"\r\n", 1)[0]:
            raise RuntimeError(f"WebSocket handshake failed: {response[:200]!r}")
        headers = {}
        for line in bytes(response).split(b"\r\n")[1:]:
            if b":" in line:
                key_name, value = line.split(b":", 1)
                headers[key_name.strip().lower()] = value.strip()
        expected = base64.b64encode(
            hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()
        )
        if headers.get(b"sec-websocket-accept") != expected:
            raise RuntimeError("WebSocket handshake accept key mismatch")
        sock.settimeout(self.timeout)
        self._socket = sock
        return self

    def __exit__(self, *_args):
        if self._socket:
            self._socket.close()
        self._socket = None

    def _send_frame(self, opcode: int, data: bytes) -> None:
        assert self._socket
        mask = os.urandom(4)
        header = bytearray([0x80 | opcode])
        length = len(data)
        if length < 126:
            header.append(0x80 | length)
        elif length < 65536:
            header += bytes([0x80 | 126]) + struct.pack("!H", length)
        else:
            header += bytes([0x80 | 127]) + struct.pack("!Q", length)
        masked = bytes(byte ^ mask[index % 4] for index, byte in enumerate(data))
        self._socket.sendall(bytes(header) + mask + masked)

    def _send(self, payload: dict) -> None:
        self._send_frame(0x1, json.dumps(payload).encode())

    def _receive(self) -> dict:
        assert self._socket
        fragments = bytearray()
        message_opcode = None
        while True:
            first, second = _read_exact(self._socket, 2)
            final = bool(first & 0x80)
            opcode = first & 0x0F
            length = second & 0x7F
            if length == 126:
                length = struct.unpack("!H", _read_exact(self._socket, 2))[0]
            elif length == 127:
                length = struct.unpack("!Q", _read_exact(self._socket, 8))[0]
            mask = _read_exact(self._socket, 4) if second & 0x80 else None
            data = _read_exact(self._socket, length)
            if mask:
                data = bytes(byte ^ mask[index % 4] for index, byte in enumerate(data))
            if opcode == 0x8:
                raise RuntimeError("WebSocket closed")
            if opcode == 0x9:
                self._send_frame(0xA, data)
                continue
            if opcode == 0xA:
                continue
            if opcode in (0x1, 0x2):
                message_opcode = opcode
                fragments = bytearray(data)
            elif opcode == 0x0 and message_opcode is not None:
                fragments.extend(data)
            else:
                continue
            if final:
                if message_opcode != 0x1:
                    raise RuntimeError("Unexpected binary CDP frame")
                return json.loads(bytes(fragments).decode())

    def call(self, method: str, params: dict | None = None) -> dict:
        self._next_id += 1
        request_id = self._next_id
        self._send({"id": request_id, "method": method, "params": params or {}})
        deadline = time.time() + self.timeout
        while time.time() < deadline:
            response = self._receive()
            if response.get("id") == request_id:
                if "error" in response:
                    raise RuntimeError(f"CDP {method}: {response['error']}")
                return response.get("result", {})
        raise RuntimeError(f"CDP call timed out: {method}")

    def evaluate(self, expression: str) -> Any:
        result = self.call(
            "Runtime.evaluate",
            {"expression": expression, "returnByValue": True, "awaitPromise": True},
        ).get("result", {})
        if result.get("subtype") == "error":
            raise RuntimeError(result.get("description", "Runtime.evaluate failed"))
        return result.get("value")

    def screenshot(self) -> bytes:
        result = self.call(
            "Page.captureScreenshot",
            {"format": "png", "fromSurface": True, "captureBeyondViewport": False},
        )
        return base64.b64decode(result["data"])


class AndroidSession:
    def __init__(
        self,
        serial: str | None = None,
        port: int = 9333,
        package: str = DEFAULT_PACKAGE,
        activity: str = DEFAULT_ACTIVITY,
    ):
        self.serial = serial or None
        self.port = port
        self.package = package
        self.activity = activity
        self.adb = adb_command(self.serial)

    def shell(self, *args: str, check: bool = True) -> str:
        return run(self.adb + ["shell", *args], check=check).strip().replace("\r", "")

    def ensure_device(self) -> None:
        state = run(self.adb + ["get-state"], check=False).strip()
        if state != "device":
            raise RuntimeError(f"device not ready: {state!r}")

    def install(self, apk: Path) -> None:
        run(self.adb + ["install", "-r", str(apk)])

    def clear_app(self) -> None:
        self.shell("pm", "clear", self.package)

    def start_app(self) -> None:
        self.shell("am", "start", "-n", self.activity)

    def open_file(self, path: str, mime: str | None = None) -> None:
        self.shell("am", "force-stop", self.package, check=False)
        command = ["am", "start", "-a", "android.intent.action.VIEW", "-d", f"file://{path}"]
        if mime:
            command += ["-t", mime]
        command += ["-n", self.activity]
        self.shell(*command)

    def grant_file_access(self) -> None:
        self.shell("appops", "set", self.package, "MANAGE_EXTERNAL_STORAGE", "allow", check=False)
        self.shell("appops", "set", self.package, "READ_EXTERNAL_STORAGE", "allow", check=False)

    def forward_webview(self, timeout: int = 30) -> None:
        deadline = time.time() + timeout
        while time.time() < deadline:
            pid = self.shell("pidof", self.package, check=False)
            if pid:
                run(self.adb + ["forward", "--remove", f"tcp:{self.port}"], check=False)
                run(
                    self.adb
                    + [
                        "forward",
                        f"tcp:{self.port}",
                        f"localabstract:webview_devtools_remote_{pid}",
                    ]
                )
                return
            time.sleep(0.5)
        raise RuntimeError(f"app process did not start: {self.package}")

    def websocket_url(self, timeout: int = 20) -> str:
        deadline = time.time() + timeout
        last_error = ""
        while time.time() < deadline:
            try:
                with urllib.request.urlopen(f"http://127.0.0.1:{self.port}/json", timeout=2) as response:
                    pages = json.load(response)
                if pages:
                    return pages[0]["webSocketDebuggerUrl"]
            except Exception as error:
                last_error = str(error)
                time.sleep(0.25)
        raise RuntimeError(f"CDP not ready on port {self.port}: {last_error}")

    def cdp(self, timeout: int = 15) -> CdpClient:
        self.forward_webview()
        return CdpClient(self.websocket_url(), timeout)

    def profile(self) -> dict[str, str]:
        webview = self.shell("dumpsys", "package", "com.google.android.webview", check=False)
        webview_version = "unknown"
        for line in webview.splitlines():
            if "versionName=" in line:
                webview_version = line.split("versionName=", 1)[1].strip()
                break
        app = self.shell("dumpsys", "package", self.package, check=False)
        app_version = "unknown"
        for line in app.splitlines():
            if "versionName=" in line:
                app_version = line.split("versionName=", 1)[1].strip()
                break
        return {
            "serial": self.serial or run(self.adb + ["get-serialno"]).strip(),
            "model": self.shell("getprop", "ro.product.model"),
            "android": self.shell("getprop", "ro.build.version.release"),
            "sdk": self.shell("getprop", "ro.build.version.sdk"),
            "size": self.shell("wm", "size"),
            "density": self.shell("wm", "density"),
            "font_scale": self.shell("settings", "get", "system", "font_scale"),
            "locale": self.shell("getprop", "persist.sys.locale"),
            "ui_mode": self.shell("cmd", "uimode", "night", check=False),
            "build_fingerprint": self.shell("getprop", "ro.build.fingerprint"),
            "webview_version": webview_version,
            "app_version": app_version,
        }

    def file_sha256(self, path: str) -> str:
        output = self.shell("sha256sum", path)
        return output.split()[0] if output else ""


def write_bytes(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
