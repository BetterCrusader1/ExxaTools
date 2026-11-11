"""ExxaTools - a manifest and depot manager for the Linux Steam client.

Front end for Vapor Forge: resolve Steam apps, pin depot manifests, store depot
decryption keys, and emit Lua plans into Vapor Forge's script directory.
"""

__version__ = "1.0.0"

from . import http, lua, providers, server, steam, store, vfsys  # noqa: F401

__all__ = ["__version__", "http", "lua", "providers", "server", "steam", "store", "vfsys"]
