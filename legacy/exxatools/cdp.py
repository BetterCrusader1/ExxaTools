"""A very small Chrome DevTools Protocol client.

Steam's UI is Chromium. With ``-cef-enable-debugging`` it exposes the CDP
endpoint on localhost, which is the only supported way to run our own script
inside the client's main window. No third-party packages: the websocket layer
here is just the handful of frames CDP needs (text, ping, close).
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
import socket
import struct
import urllib.request
from urllib.error import URLError

GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


class CdpError(RuntimeError):
    pass


class WebSocket:
    def __init__(self, url: str, timeout: float = 10.0) -> None:
        if not url.startswith("ws://"):
            raise CdpError(f"unsupported websocket url: {url}")
        rest = url[5:]
        host_port, _, path = rest.partition("/")
        host, _, port = host_port.partition(":")
        self.sock = socket.create_connection((host or "127.0.0.1", int(port or 80)), timeout=timeout)
        self.sock.settimeout(timeout)
        self.buffer = b""
        key = base64.b64encode(os.urandom(16)).decode()
        request = (
            f"GET /{path} HTTP/1.1\r\n"
            f"Host: {host_port}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\n"
            "Sec-WebSocket-Version: 13\r\n\r\n"
        )
        self.sock.sendall(request.encode())
        header = self._read_until(b"\r\n\r\n")
        if b" 101 " not in header.split(b"\r\n", 1)[0]:
            raise CdpError(f"websocket handshake failed: {header[:120]!r}")
        expected = base64.b64encode(hashlib.sha1((key + GUID).encode()).digest())
        if expected not in header:
            raise CdpError("websocket handshake key mismatch")

    def _read_until(self, marker: bytes) -> bytes:
        while marker not in self.buffer:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise CdpError("connection closed during handshake")
            self.buffer += chunk
        head, _, self.buffer = self.buffer.partition(marker)
        return head

    def _read_exact(self, size: int) -> bytes:
        while len(self.buffer) < size:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise CdpError("connection closed")
            self.buffer += chunk
        data, self.buffer = self.buffer[:size], self.buffer[size:]
        return data

    def send(self, text: str) -> None:
        payload = text.encode()
        header = bytearray([0x81])
        length = len(payload)
        if length < 126:
            header.append(0x80 | length)
        elif length < 1 << 16:
            header.append(0x80 | 126)
            header += struct.pack(">H", length)
        else:
            header.append(0x80 | 127)
            header += struct.pack(">Q", length)
        mask = os.urandom(4)
        header += mask
        masked = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))
        self.sock.sendall(bytes(header) + masked)

    def recv(self) -> str | None:
        while True:
            first, second = self._read_exact(2)
            fin = first & 0x80
            opcode = first & 0x0F
            length = second & 0x7F
            if length == 126:
                (length,) = struct.unpack(">H", self._read_exact(2))
            elif length == 127:
                (length,) = struct.unpack(">Q", self._read_exact(8))
            masked = second & 0x80
            mask = self._read_exact(4) if masked else b""
            payload = self._read_exact(length) if length else b""
            if masked:
                payload = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))
            if opcode == 0x9:  # ping
                self._send_control(0xA, payload)
                continue
            if opcode == 0xA:  # pong
                continue
            if opcode == 0x8:  # close
                return None
            if opcode in (0x1, 0x2):
                if fin:
                    return payload.decode("utf-8", "replace")
                chunks = [payload]
                while True:
                    more, text = self._recv_frame()
                    chunks.append(text)
                    if more:
                        break
                return b"".join(chunks).decode("utf-8", "replace")

    def _recv_frame(self) -> tuple[bool, bytes]:
        first, second = self._read_exact(2)
        length = second & 0x7F
        if length == 126:
            (length,) = struct.unpack(">H", self._read_exact(2))
        elif length == 127:
            (length,) = struct.unpack(">Q", self._read_exact(8))
        payload = self._read_exact(length) if length else b""
        return bool(first & 0x80), payload

    def _send_control(self, opcode: int, payload: bytes) -> None:
        header = bytearray([0x80 | opcode, 0x80 | len(payload)])
        mask = os.urandom(4)
        header += mask
        masked = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))
        self.sock.sendall(bytes(header) + masked)

    def call(self, method: str, params: dict | None = None, message_id: int = 1) -> dict:
        self.send(json.dumps({"id": message_id, "method": method, "params": params or {}}))
        while True:
            raw = self.recv()
            if raw is None:
                raise CdpError("connection closed while waiting for a reply")
            try:
                message = json.loads(raw)
            except json.JSONDecodeError:
                continue
            if message.get("id") != message_id:
                continue
            if "error" in message:
                raise CdpError(str(message["error"]))
            return message.get("result", {})

    def close(self) -> None:
        try:
            self._send_control(0x8, b"")
        except OSError:
            pass
        try:
            self.sock.close()
        except OSError:
            pass


def version(port: int = 8080, timeout: float = 3.0) -> dict | None:
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/version", timeout=timeout) as response:
            return json.loads(response.read().decode("utf-8", "replace"))
    except (URLError, OSError, json.JSONDecodeError):
        return None


def targets(port: int = 8080, timeout: float = 3.0) -> list[dict]:
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list", timeout=timeout) as response:
            data = json.loads(response.read().decode("utf-8", "replace"))
    except (URLError, OSError, json.JSONDecodeError):
        return []
    return data if isinstance(data, list) else []


def evaluate(ws_url: str, expression: str, timeout: float = 10.0):
    socket_ = WebSocket(ws_url, timeout=timeout)
    try:
        result = socket_.call(
            "Runtime.evaluate",
            {"expression": expression, "returnByValue": True, "awaitPromise": True},
        )
    finally:
        socket_.close()
    if "exceptionDetails" in result:
        raise CdpError(str(result["exceptionDetails"].get("text") or result["exceptionDetails"]))
    return result.get("result", {}).get("value")