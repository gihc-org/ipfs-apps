#!/usr/bin/env python3
"""WebSocket-delen af Loft-smoke-testen.

Forbinder to gæster til samme loft og verificerer protokollen fra README:
join → roster, join-broadcast, media-state/signal med server-stemplet `from`,
at signaler til ukendte modtagere droppes, og at leave annonceres.

Brug:
    python3 scripts/smoke-ws.py <ws_origin> <loft_id> [--insecure]
    python3 scripts/smoke-ws.py wss://loft.test.gihc.online <id> \
        --connect-ip 65.109.233.92 --sni loft.test.gihc.online

<ws_origin> er origin uden path, fx `wss://loft.test.gihc.online`.
`--connect-ip` bruges når A-recorden endnu ikke er slået igennem i den lokale
resolver: der forbindes til IP'en, mens SNI og certifikat-verifikation stadig
bruger værtsnavnet (`--sni`, default værtsnavnet fra origin).
Koden er skrevet til websockets 10.x (samme API-flade bruges i CI-imaget).
"""

import argparse
import asyncio
import json
import socket
import ssl
import sys
import time
import urllib.parse
import uuid

import websockets


class SmokeError(Exception):
    pass


def ok(label: str) -> None:
    print(f"ok  {label}")


def ssl_context(insecure: bool) -> ssl.SSLContext:
    context = ssl.create_default_context()
    if insecure:
        context.check_hostname = False
        context.verify_mode = ssl.CERT_NONE
    return context


def pin_dns(host: str, ip: str) -> None:
    """Slår `host` op som `ip` i denne proces — samme effekt som curl --resolve.

    Vigtigt at kun opslaget ændres: Host-header, SNI og certifikat-verifikation
    skal fortsat bruge værtsnavnet, ellers svarer ingress-nginx 404.
    """
    real_getaddrinfo = socket.getaddrinfo

    def patched(name, port, family=0, type=0, proto=0, flags=0):
        if name == host:
            name = ip
            family = socket.AF_UNSPEC
        return real_getaddrinfo(name, port, family, type, proto, flags)

    socket.getaddrinfo = patched


async def recv_json(ws, timeout: float = 10.0) -> dict:
    try:
        raw = await asyncio.wait_for(ws.recv(), timeout)
    except asyncio.TimeoutError as exc:
        raise SmokeError(f"ingen besked inden for {timeout:g}s") from exc
    return json.loads(raw)


async def wait_for(ws, kinds: tuple[str, ...], timeout: float = 10.0, where=None) -> dict:
    """Læser beskeder indtil en af `kinds` (evt. filtreret af `where`) dukker op."""
    deadline = time.monotonic() + timeout
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise SmokeError(f"ventede forgæves på {kinds} i {timeout:g}s")
        msg = await recv_json(ws, remaining)
        if msg.get("type") in kinds and (where is None or where(msg)):
            return msg


async def expect_silence(ws, kinds: tuple[str, ...], timeout: float = 2.0) -> None:
    """Fejler hvis en besked af typen `kinds` ankommer inden for `timeout`."""
    deadline = time.monotonic() + timeout
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return
        try:
            msg = await recv_json(ws, remaining)
        except SmokeError:
            return
        if msg.get("type") in kinds:
            raise SmokeError(f"uventet {msg.get('type')}-besked: {msg}")


async def connect(url: str, insecure: bool):
    kwargs = {}
    if url.startswith("wss"):
        kwargs["ssl"] = ssl_context(insecure)
    return await websockets.connect(url, **kwargs)


async def join(url: str, name: str, insecure: bool):
    ws = await connect(url, insecure)
    await ws.send(json.dumps({"type": "join", "name": name}))
    roster = await wait_for(ws, ("roster",))
    if roster.get("self", {}).get("name") != name:
        raise SmokeError(f"roster.self ser forkert ud: {roster}")
    if not any(p.get("name") == name for p in roster.get("participants", [])):
        raise SmokeError(f"roster.participants mangler selv: {roster}")
    return ws, roster["self"]


async def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("origin", help="ws/wss-origin uden path")
    parser.add_argument("loft_id")
    parser.add_argument(
        "--insecure", action="store_true", help="spring TLS-verifikation over (staging-cert)"
    )
    parser.add_argument(
        "--connect-ip", help="forbind til denne IP i stedet for at slå værtsnavnet op"
    )
    parser.add_argument(
        "--sni", help="værtsnavn til SNI/certifikat-verifikation sammen med --connect-ip"
    )
    args = parser.parse_args()

    url = f"{args.origin.rstrip('/')}/v1/ws/{args.loft_id}"
    if args.connect_ip:
        host = args.sni or urllib.parse.urlparse(url).hostname
        pin_dns(host, args.connect_ip)
    suffix = uuid.uuid4().hex[:6]

    alice, alice_self = await join(url, f"smoke-a-{suffix}", args.insecure)
    ok("WS join → roster (self + participants)")

    bob, bob_self = await join(url, f"smoke-b-{suffix}", args.insecure)
    ok("WS join → roster for anden deltager")

    join_msg = await wait_for(
        alice,
        ("join",),
        where=lambda m: m.get("participant", {}).get("id") == bob_self["id"],
    )
    if join_msg["participant"]["name"] != bob_self["name"]:
        raise SmokeError(f"join-navn matcher ikke: {join_msg}")
    ok("WS join broadcastes til loftet")

    await bob.send(
        json.dumps({"type": "media-state", "audio": True, "video": False, "screen": False})
    )
    media = await wait_for(alice, ("media-state",))
    if media.get("from", {}).get("id") != bob_self["id"]:
        raise SmokeError(f"media-state uden korrekt server-stemplet from: {media}")
    ok("media-state relayes med server-stemplet from")

    await bob.send(
        json.dumps(
            {
                "type": "signal",
                "to": alice_self["id"],
                "signal": {"type": "offer", "sdp": "smoke"},
            }
        )
    )
    signal = await wait_for(alice, ("signal",), where=lambda m: m.get("to") == alice_self["id"])
    if signal.get("from", {}).get("name") != bob_self["name"] or "sdp" not in signal.get("signal", {}):
        raise SmokeError(f"signal blev ikke videresendt korrekt: {signal}")
    ok("signal videresendes til angivet modtager")

    bogus = str(uuid.uuid4())
    await bob.send(
        json.dumps({"type": "signal", "to": bogus, "signal": {"type": "offer", "sdp": "bogus"}})
    )
    await expect_silence(alice, ("signal",), timeout=2.0)
    ok("signal til ukendt modtager droppes")

    await bob.close()
    await wait_for(
        alice,
        ("leave",),
        where=lambda m: m.get("participant", {}).get("id") == bob_self["id"],
    )
    ok("leave annonceres når forbindelsen lukkes")

    await alice.close()
    return 0


if __name__ == "__main__":
    try:
        sys.exit(asyncio.run(main()))
    except SmokeError as exc:
        print(f"FAIL: WebSocket-flow — {exc}", file=sys.stderr)
        sys.exit(1)
    except OSError as exc:
        print(f"FAIL: WebSocket-forbindelse — {exc}", file=sys.stderr)
        sys.exit(1)
