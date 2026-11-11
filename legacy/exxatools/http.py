"""Minimal HTTP helpers on top of urllib, with a real client identity."""

from __future__ import annotations

import gzip
import json
import urllib.error
import urllib.parse
import urllib.request
import zlib

USER_AGENT = "ExxaTools/1.0 (Linux; +https://github.com/Hintay/vapor-forge)"
DEFAULT_TIMEOUT = 20.0


class HttpError(RuntimeError):
    """Raised for every transport, status, or decode failure."""

    def __init__(self, message: str, status: int | None = None, body: str = "") -> None:
        super().__init__(message)
        self.status = status
        self.body = body


def _decode(payload: bytes, encoding: str | None) -> bytes:
    if not payload:
        return payload
    try:
        if encoding == "gzip":
            return gzip.decompress(payload)
        if encoding == "deflate":
            return zlib.decompress(payload)
    except (OSError, zlib.error):
        return payload
    return payload


def request(
    url: str,
    *,
    data: bytes | None = None,
    headers: dict[str, str] | None = None,
    timeout: float = DEFAULT_TIMEOUT,
    method: str | None = None,
) -> bytes:
    merged = {
        "User-Agent": USER_AGENT,
        "Accept": "*/*",
        "Accept-Encoding": "gzip, deflate",
    }
    if headers:
        merged.update(headers)
    req = urllib.request.Request(url, data=data, headers=merged, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as response:
            return _decode(response.read(), response.headers.get("Content-Encoding"))
    except urllib.error.HTTPError as error:
        body = b""
        try:
            body = error.read(4096)
        except OSError:
            pass
        raise HttpError(
            f"{url} returned HTTP {error.code}",
            status=error.code,
            body=body.decode("utf-8", "replace"),
        ) from error
    except urllib.error.URLError as error:
        raise HttpError(f"{url} failed: {error.reason}") from error
    except TimeoutError as error:
        raise HttpError(f"{url} timed out") from error


def get_text(
    url: str,
    *,
    headers: dict[str, str] | None = None,
    timeout: float = DEFAULT_TIMEOUT,
) -> str:
    return request(url, headers=headers, timeout=timeout).decode("utf-8", "replace")


def get_json(
    url: str,
    *,
    headers: dict[str, str] | None = None,
    timeout: float = DEFAULT_TIMEOUT,
):
    text = get_text(url, headers=headers, timeout=timeout)
    try:
        return json.loads(text)
    except json.JSONDecodeError as error:
        raise HttpError(f"{url} returned non-JSON payload", body=text[:400]) from error


def quote(value: str) -> str:
    return urllib.parse.quote(value, safe="")
