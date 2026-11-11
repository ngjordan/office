#!/usr/bin/env python3
"""Rewrite the Office suite's WiX installer so it does not take over foreign file types.

A non-advertised <Extension> writes HKCR\\.pdf\\(Default) = the ProgId. That makes the
suite the default handler for every format it knows, whether the user asked for it or
not, and uninstalling deletes the value and breaks whatever was there before.

So: <Extension> only for the suite's own format. Every other type is offered as
"Open with" (OpenWithProgids) and in Settings > Default apps (Capabilities), which is
what a user who wants the suite for .pdf can pick.

Run:  python3 packaging/windows/make_wxs.py
"""

from __future__ import annotations

import re
from pathlib import Path

HERE = Path(__file__).parent
WXS = HERE / "iridium-office.wxs"

# The formats the suite owns. Everything else is a foreign type it offers but does not own.
NATIVE = {"iridium-decks"}

# extension -> (ProgId, the description shown in "Open with")
FORMATS: dict[str, tuple[str, str]] = {}


def read_formats() -> None:
    """Read the extension -> (ProgId, description) table out of the current .wxs."""
    text = WXS.read_text()
    for prog_id, body in re.findall(r'<ProgId Id="([^"]+)"[^>]*>(.*?)</ProgId>', text, re.S):
        desc = re.search(r'Description="([^"]+)"', body)
        for ext in re.findall(r'<Extension Id="([^"]+)"', body):
            FORMATS[ext] = (prog_id, desc.group(1) if desc else prog_id)


def progid_entries() -> str:
    """One registry key per ProgId, with its icon and open command."""
    seen: dict[str, str] = {}
    for ext, (prog_id, desc) in sorted(FORMATS.items()):
        seen.setdefault(prog_id, desc)
    out = []
    for prog_id, desc in sorted(seen.items()):
        out.append(f"""
        <RegistryKey Root="HKCR" Key="{prog_id}">
          <RegistryValue Type="string" Value="{desc}" />
          <RegistryValue Key="DefaultIcon" Type="string" Value="&quot;[#OfficeExe]&quot;,0" />
          <RegistryValue Key="shell\\open\\command" Type="string" Value="&quot;[#OfficeExe]&quot; &quot;%1&quot;" />
        </RegistryKey>""")
    return "\n".join(out)


def openwith_entries() -> str:
    """Every foreign extension gets an OpenWithProgids row."""
    out = []
    for ext, (prog_id, _) in sorted(FORMATS.items()):
        if ext in NATIVE:
            continue
        out.append(
            f'        <RegistryValue Root="HKCR" Key=".{ext}\\OpenWithProgids" '
            f'Name="{prog_id}" Type="string" Value="" />'
        )
    return "\n".join(out)


def capabilities() -> str:
    """Settings > Default apps: lets the user pick the suite per type."""
    rows = "\n".join(
        f'          <RegistryValue Key="FileAssociations" Name=".{ext}" Type="string" Value="{prog_id}" />'
        for ext, (prog_id, _) in sorted(FORMATS.items())
        if ext not in NATIVE
    )
    return f"""
        <RegistryKey Root="HKMU" Key="Software\\Office\\Capabilities">
          <RegistryValue Name="ApplicationName" Type="string" Value="Office" />
          <RegistryValue Name="ApplicationDescription" Type="string" Value="Office suite" />
{rows}
        </RegistryKey>
        <RegistryValue Root="HKMU" Key="Software\\RegisteredApplications" Name="Office" Type="string" Value="Software\\Office\\Capabilities" />"""


def native_extension() -> str:
    """The one type the suite owns, kept as a real Extension so it is the default handler."""
    out = []
    for ext, (prog_id, desc) in sorted(FORMATS.items()):
        if ext not in NATIVE:
            continue
        out.append(f"""
        <ProgId Id="{prog_id}" Description="{desc}">
          <Extension Id="{ext}" ContentType="application/x-{ext}">
            <Verb Id="open" Command="Open" TargetFile="OfficeExe" Argument="&quot;%1&quot;" />
          </Extension>
        </ProgId>""")
    return "\n".join(out)


def main() -> None:
    read_formats()
    if not FORMATS:
        raise SystemExit("no extensions found in the .wxs")
    text = WXS.read_text()

    # Replace the whole ProgId block with the new structure.
    start = text.index("        <ProgId Id=")
    end = text.index("<!-- \"Open with\" for every format")
    text = text[:start] + native_extension().lstrip("\n") + "\n" + text[end:]

    # Replace the "Open with" registry block with the new structure.
    start = text.index("<!-- \"Open with\" for every format")
    end = text.index('        <RegistryValue Root="HKMU" Key="Software\\Microsoft\\Windows\\CurrentVersion\\App Paths')
    block = (
        "        <!-- The suite's own types are registered as Extensions above. Every other type\n"
        "             is offered, not imposed: an OpenWithProgids row puts it in \"Open with\",\n"
        "             and the Capabilities block below lets the user pick it in the default\n"
        "             apps settings. Writing HKCR\\.pdf\\(Default) would take the file type over\n"
        "             from whatever the user already had. -->\n"
        + progid_entries() + "\n"
        + openwith_entries() + "\n"
        + capabilities() + "\n\n"
    )
    text = text[:start] + block + text[end:]

    # The App Paths entry stays: it is what Win+R and ShellExecute use.
    WXS.write_text(text)
    print(f"rewrote {WXS.name}: {len(FORMATS)} formats, "
          f"{len(FORMATS) - len(NATIVE)} offered as \"Open with\", "
          f"{len(NATIVE)} owned")


if __name__ == "__main__":
    main()
