"""Tiny VDF (Valve KeyValues) reader/editor for Steam launch options.

Only the paths ExxaTools cares about are touched, and only by rewriting the
single line that holds a value. The rest of the file keeps its bytes.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field


class VdfError(RuntimeError):
    pass


@dataclass
class Node:
    key: str | None = None
    value: str | None = None
    key_start: int = 0
    value_end: int = 0
    open_brace: int | None = None
    close_brace: int | None = None
    children: list["Node"] = field(default_factory=list)

    def child(self, key: str) -> "Node | None":
        for item in self.children:
            if item.key == key:
                return item
        return None


def _tokenize(text: str):
    """Yield (kind, start, end, value) where kind is str, {, or }."""
    index = 0
    size = len(text)
    while index < size:
        char = text[index]
        if char in " \t\r\n":
            index += 1
            continue
        if text.startswith("//", index):
            newline = text.find("\n", index)
            index = size if newline == -1 else newline
            continue
        if char == "{":
            yield ("{", index, index + 1, None)
            index += 1
            continue
        if char == "}":
            yield ("}", index, index + 1, None)
            index += 1
            continue
        if char == '"':
            cursor = index + 1
            chunks = []
            while cursor < size:
                if text[cursor] == "\\" and cursor + 1 < size:
                    chunks.append(text[cursor : cursor + 2])
                    cursor += 2
                    continue
                if text[cursor] == '"':
                    break
                chunks.append(text[cursor])
                cursor += 1
            yield ("str", index, cursor + 1, "".join(chunks))
            index = cursor + 1
            continue
        cursor = index
        while cursor < size and text[cursor] not in ' \t\r\n{}"':
            cursor += 1
        if cursor == index:
            raise VdfError(f"unexpected byte at offset {index}")
        yield ("str", index, cursor, text[index:cursor])
        index = cursor


def parse(text: str) -> Node:
    root = Node()
    stack = [root]
    pending: tuple[str, int, int] | None = None

    for kind, start, end, value in _tokenize(text):
        current = stack[-1]
        if kind == "str":
            if pending is None:
                pending = (value or "", start, end)
            else:
                current.children.append(
                    Node(key=pending[0], value=value, key_start=pending[1], value_end=end)
                )
                pending = None
            continue
        if kind == "{":
            if pending is None:
                raise VdfError(f"block without a key at offset {start}")
            node = Node(key=pending[0], key_start=pending[1], open_brace=start, value_end=end)
            current.children.append(node)
            stack.append(node)
            pending = None
            continue
        if pending is not None:
            current.children.append(
                Node(key=pending[0], key_start=pending[1], value_end=pending[2])
            )
            pending = None
        if len(stack) == 1:
            raise VdfError(f"unbalanced closing brace at offset {start}")
        stack.pop().close_brace = start

    if pending is not None:
        root.children.append(Node(key=pending[0], key_start=pending[1], value_end=pending[2]))
    if len(stack) != 1:
        raise VdfError("unbalanced braces in VDF file")
    return root


def find(root: Node, path: list[str]) -> Node | None:
    node = root
    for key in path:
        node = node.child(key)
        if node is None:
            return None
    return node


def _line_indent(text: str, position: int) -> str:
    start = text.rfind("\n", 0, position) + 1
    indent = []
    for char in text[start:position]:
        if char in "\t ":
            indent.append(char)
        else:
            break
    return "".join(indent)


def _children_indent(text: str, block: Node) -> str:
    for child in block.children:
        return _line_indent(text, child.key_start)
    return _line_indent(text, block.open_brace or 0) + "\t"


def _replace_line(text: str, node: Node, key: str, value: str) -> str:
    line_start = text.rfind("\n", 0, node.key_start) + 1
    line_end = text.find("\n", node.value_end)
    if line_end == -1:
        line_end = len(text)
    indent = _line_indent(text, node.key_start)
    return text[:line_start] + f'{indent}"{key}"\t\t"{value}"' + text[line_end:]


def _insert_line(text: str, block: Node, line: str) -> str:
    if block.close_brace is None:
        raise VdfError("cannot insert into an unterminated block")
    close_start = text.rfind("\n", 0, block.close_brace) + 1
    indent = _line_indent(text, block.close_brace)
    return text[:close_start] + f"{indent}{line}\n" + text[close_start:]


STEAM_APPS_PATH = ["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"]


def read_launch_options(text: str, appid: int) -> str | None:
    root = parse(text)
    apps = find(root, STEAM_APPS_PATH)
    if apps is None:
        return None
    app = apps.child(str(appid))
    if app is None:
        return None
    options = app.child("LaunchOptions")
    return options.value if options else None


def set_launch_options(text: str, appid: int, options: str) -> tuple[str, str | None]:
    """Return the new text and the previous options value."""
    root = parse(text)
    apps = find(root, STEAM_APPS_PATH)
    if apps is None or apps.open_brace is None:
        raise VdfError("this config has no Steam apps block")

    app = apps.child(str(appid))
    if app is not None:
        previous_node = app.child("LaunchOptions")
        previous = previous_node.value if previous_node else None
        if previous_node is not None:
            return _replace_line(text, previous_node, "LaunchOptions", options), previous
        return _insert_line(text, app, f'"LaunchOptions"\t\t"{options}"'), previous

    child_indent = _children_indent(text, apps)
    inner = child_indent + "\t"
    block = (
        f'"{appid}"\n'
        f"{child_indent}{{\n"
        f'{inner}"LaunchOptions"\t\t"{options}"\n'
        f"{child_indent}}}"
    )
    return _insert_line(text, apps, block), None


def merge_launch_option(current: str | None, flag: str) -> str:
    text = (current or "").strip()
    if not flag:
        return text
    if re.search(rf"(^|\s){re.escape(flag)}(\s|$)", text):
        return text
    return f"{text} {flag}".strip()
