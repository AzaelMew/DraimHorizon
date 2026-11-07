"""In-memory WebSocket routing and optional, token-protected translation."""
from __future__ import annotations

import argparse
import asyncio
from collections import Counter
from contextlib import suppress
from dataclasses import dataclass, field
import hashlib
import hmac
import ipaddress
from http import HTTPStatus
import json
import logging
import os
import re
import secrets
import signal
import time
from urllib.error import HTTPError, URLError
from urllib.request import HTTPRedirectHandler, Request, build_opener

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
from websockets.asyncio.server import ServerConnection, serve
from websockets.exceptions import ConnectionClosed

from . import __version__

AUTH_PREFIX = b"DHRelay-auth-v1\0"
HEX = re.compile(r"[0-9a-f]+\Z")
NAME = re.compile(r"[A-Za-z0-9_]{1,16}\Z")
MAX_FRAME = 16384
TRANSLATE_PATH = "/v1/translate"
DEEPL_ENDPOINT = "https://api-free.deepl.com/v2/translate"
TRANSLATION_BURST = 1200
TRANSLATION_RATE = 10  # characters per second across the relay


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def translate_deepl(key: str, text: str, target: str) -> str:
    request = Request(
        DEEPL_ENDPOINT,
        data=json.dumps({"text": [text], "target_lang": target}, ensure_ascii=False).encode("utf-8"),
        headers={"Authorization": "DeepL-Auth-Key " + key, "Content-Type": "application/json"},
        method="POST",
    )
    with build_opener(NoRedirect()).open(request, timeout=4) as response:
        data = response.read(65537)
    if len(data) > 65536:
        raise ValueError("response too large")
    result = json.loads(data)["translations"][0]["text"]
    if not isinstance(result, str) or not result.strip() or len(result.encode("utf-8")) > 8192:
        raise ValueError("invalid translation")
    return result


def hex_bytes(value: object, length: int) -> bytes:
    if not isinstance(value, str) or len(value) != length * 2 or not HEX.fullmatch(value):
        raise ValueError("invalid hex")
    return bytes.fromhex(value)


def encode(frame: list) -> str:
    return json.dumps(frame, ensure_ascii=False, separators=(",", ":"))


def parse(raw: str | bytes) -> list:
    if not isinstance(raw, str) or len(raw.encode("utf-8")) > MAX_FRAME:
        raise ValueError("invalid frame")
    value = json.loads(raw)
    if not isinstance(value, list):
        raise ValueError("expected array")
    return value


@dataclass(frozen=True)
class Limits:
    total: int = 0
    realm: int = 0
    ip: int = 0
    queue: int = 128
    auth_timeout: float = 10.0
    rate: float = 0.0
    burst: int = 20


@dataclass(eq=False)
class Peer:
    ws: ServerConnection
    realm: str
    code: str
    key: bytes
    cert: str
    signature: str
    queue: asyncio.Queue = field(default_factory=asyncio.Queue)
    tokens: float = 20
    updated: float = field(default_factory=time.monotonic)


class Relay:
    def __init__(self, access_token: str = "", limits: Limits = Limits(),
                 trusted_proxy_cidrs: str = "127.0.0.0/8,::1/128", deepl_key: str = ""):
        self.access_token = access_token
        if deepl_key and (not access_token or len(deepl_key) > 256
                          or not all(33 <= ord(c) <= 126 for c in deepl_key)):
            raise ValueError("DeepL requires a valid local key and RELAY_ACCESS_TOKEN")
        self.deepl_key = deepl_key
        self.translation_active = 0
        self.translation_tokens = TRANSLATION_BURST
        self.translation_updated = time.monotonic()
        self.translation_retry = 0.0
        self.trusted_proxies = tuple(ipaddress.ip_network(value.strip(), strict=True)
                                     for value in trusted_proxy_cidrs.split(",") if value.strip())
        self.limits = limits
        self.peers: dict[bytes, Peer] = {}
        self.codes: dict[tuple[str, str], Peer] = {}
        self.connections = 0
        self.ips: Counter = Counter()
        self.realms: Counter = Counter()

    async def process_request(self, connection, request):
        if request.path == "/healthz":
            return connection.respond(HTTPStatus.OK, encode(["ok", __version__]) + "\n")
        if request.path == TRANSLATE_PATH:
            if not self.deepl_key or not self.access_token:
                return connection.respond(HTTPStatus.NOT_FOUND, "not found\n")
        elif request.path != "/v1/relay":
            return connection.respond(HTTPStatus.NOT_FOUND, "not found\n")
        if self.access_token:
            values = request.headers.get_all("Authorization")
            expected = ("Bearer " + self.access_token).encode()
            if len(values) != 1 or not hmac.compare_digest(values[0].encode(), expected):
                return connection.respond(HTTPStatus.UNAUTHORIZED, "unauthorized\n")
        return None

    async def translate(self, ws: ServerConnection):
        if self.translation_active >= 4 or time.monotonic() < self.translation_retry:
            await ws.send(encode(["error"]))
            return
        self.translation_active += 1
        try:
            async with asyncio.timeout(2):
                target, text = parse(await ws.recv())
            if (target not in ("RU", "EN-US") or not isinstance(text, str)
                    or not text.strip() or len(text) > 500 or text.lstrip().startswith("/")):
                raise ValueError("invalid translation request")
            now = time.monotonic()
            self.translation_tokens = min(
                TRANSLATION_BURST,
                self.translation_tokens + (now - self.translation_updated) * TRANSLATION_RATE,
            )
            self.translation_updated = now
            if len(text) > self.translation_tokens:
                await ws.send(encode(["error"]))
                return
            self.translation_tokens -= len(text)
            try:
                result = await asyncio.to_thread(translate_deepl, self.deepl_key, text, target)
            except HTTPError as error:
                self.translation_retry = time.monotonic() + (900 if error.code in (401, 403, 456) else 15)
                raise
            except (URLError, OSError, ValueError, KeyError, IndexError, TypeError):
                self.translation_retry = time.monotonic() + 15
                raise
            reply = encode(["ok", result])
            await ws.send(reply if len(reply.encode("utf-8")) <= MAX_FRAME else encode(["error"]))
        except ConnectionClosed:
            pass
        except (URLError, OSError, ValueError, KeyError, IndexError, TypeError):
            with suppress(ConnectionClosed):
                await ws.send(encode(["error"]))
        finally:
            self.translation_active -= 1

    def offer(self, peer: Peer, frame: list):
        try:
            peer.queue.put_nowait(encode(frame))
        except asyncio.QueueFull:
            # One bounded writer owns every send. Cancel overloaded receivers.
            peer.ws.transport.abort()

    async def writer(self, peer: Peer):
        while True:
            await peer.ws.send(await peer.queue.get())

    def authenticate(self, frame: list, nonce: bytes) -> tuple[str, bytes, str, str]:
        if len(frame) != 4 or frame[0] != "auth" or not isinstance(frame[1], str):
            raise ValueError("invalid auth")
        cert, signature, proof = frame[1:]
        if len(cert.encode("utf-8")) > 1024:
            raise ValueError("certificate too large")
        identity = parse(cert)
        if len(identity) != 6 or identity[0] != "id1":
            raise ValueError("invalid certificate")
        _, realm, edhex, xhex, uuid, name = identity
        hex_bytes(realm, 32)
        edkey = hex_bytes(edhex, 32)
        hex_bytes(xhex, 32)
        hex_bytes(uuid, 16)
        if not isinstance(name, str) or not NAME.fullmatch(name):
            raise ValueError("invalid name")
        public = Ed25519PublicKey.from_public_bytes(edkey)
        public.verify(hex_bytes(signature, 64), cert.encode("utf-8"))
        public.verify(hex_bytes(proof, 64), AUTH_PREFIX + nonce + cert.encode("utf-8"))
        return realm, edkey, cert, signature

    def client_ip(self, ws: ServerConnection) -> str:
        address = ws.remote_address[0] if ws.remote_address else "unknown"
        try:
            remote = ipaddress.ip_address(address)
            if any(remote in network for network in self.trusted_proxies):
                values = ws.request.headers.get_all("X-Real-IP")
                if len(values) == 1 and "%" not in values[0]:
                    return str(ipaddress.ip_address(values[0]))
            return str(remote)
        except ValueError:
            return address

    async def handler(self, ws: ServerConnection):
        if ws.request.path == TRANSLATE_PATH:
            await self.translate(ws)
            return
        ip = self.client_ip(ws)
        if ((self.limits.total > 0 and self.connections >= self.limits.total)
                or (self.limits.ip > 0 and self.ips[ip] >= self.limits.ip)):
            await ws.close(1013, "connection limit")
            return
        self.connections += 1
        self.ips[ip] += 1
        peer = None
        writer = None
        try:
            nonce = secrets.token_bytes(32)
            async with asyncio.timeout(self.limits.auth_timeout):
                await ws.send(encode(["challenge", nonce.hex()]))
                realm, key, cert, signature = self.authenticate(parse(await ws.recv()), nonce)
            if key in self.peers or (self.limits.realm > 0 and self.realms[realm] >= self.limits.realm):
                await ws.close(1008, "identity or realm limit")
                return
            code = hashlib.sha256(key).digest()[:12].hex()
            if (realm, code) in self.codes:
                await ws.close(1008, "identity collision")
                return
            peer = Peer(ws, realm, code, key, cert, signature,
                        asyncio.Queue(self.limits.queue), self.limits.burst)
            existing = [p for p in self.peers.values() if p.realm == realm]
            self.peers[key] = peer
            self.codes[realm, code] = peer
            self.realms[realm] += 1
            for other in existing:
                self.offer(other, ["peer", cert, signature])
            # Stream the initial directory directly: it must not overflow the
            # bounded live queue when a realm contains more than 128 players.
            await ws.send(encode(["ready", code]))
            for other in existing:
                await ws.send(encode(["peer", other.cert, other.signature]))
            writer = asyncio.create_task(self.writer(peer))
            async for raw in ws:
                now = time.monotonic()
                peer.tokens = min(self.limits.burst, peer.tokens + (now - peer.updated) * self.limits.rate)
                peer.updated = now
                if self.limits.rate > 0:
                    if peer.tokens < 1:
                        raise ValueError("rate limit")
                    peer.tokens -= 1
                frame = parse(raw)
                if len(frame) != 3 or frame[0] != "box":
                    raise ValueError("invalid message")
                _, target, opaque = frame
                hex_bytes(target, 12)
                if not isinstance(opaque, str) or len(opaque.encode("utf-8")) > 8192:
                    raise ValueError("invalid payload")
                # Opaque encrypted box: do not decode, parse, log or store it.
                recipient = self.codes.get((realm, target))
                if recipient is not None:
                    self.offer(recipient, ["box", code, opaque])
                else:
                    self.offer(peer, ["error", "recipient_offline"])
        except (ValueError, TypeError, RecursionError, UnicodeError, InvalidSignature, TimeoutError):
            await ws.close(1008, "invalid request")
        except ConnectionClosed:
            pass
        finally:
            if writer is not None:
                writer.cancel()
                with suppress(asyncio.CancelledError, ConnectionClosed):
                    await writer
            if peer is not None and self.peers.get(peer.key) is peer:
                del self.peers[peer.key]
                self.codes.pop((peer.realm, peer.code), None)
                self.realms[peer.realm] -= 1
                if not self.realms[peer.realm]:
                    del self.realms[peer.realm]
                for other in self.peers.values():
                    if other.realm == peer.realm:
                        self.offer(other, ["gone", peer.code])
            self.connections -= 1
            self.ips[ip] -= 1
            if not self.ips[ip]:
                del self.ips[ip]

    def serve(self, host: str = "127.0.0.1", port: int = 8765):
        return serve(self.handler, host, port, process_request=self.process_request,
                     max_size=MAX_FRAME, max_queue=4, write_limit=32768,
                     compression=None, ping_interval=20, ping_timeout=20,
                     open_timeout=10, close_timeout=3, server_header=None)


async def run(host: str, port: int):
    limits = Limits(total=int(os.environ.get("RELAY_MAX_CONNECTIONS", "0")),
                    realm=int(os.environ.get("RELAY_MAX_REALM", "0")),
                    ip=int(os.environ.get("RELAY_MAX_IP", "0")),
                    rate=float(os.environ.get("RELAY_RATE", "0")))
    relay = Relay(os.environ.get("RELAY_ACCESS_TOKEN", ""), limits=limits,
                  trusted_proxy_cidrs=os.environ.get("RELAY_TRUSTED_PROXY_CIDRS",
                                                     "127.0.0.0/8,::1/128"),
                  deepl_key=os.environ.get("RELAY_DEEPL_API_KEY", "").strip())
    stopped = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        with suppress(NotImplementedError):
            loop.add_signal_handler(sig, stopped.set)
    async with relay.serve(host, port):
        logging.getLogger(__name__).info("relay listening on %s:%d", host, port)
        await stopped.wait()


def main():
    parser = argparse.ArgumentParser(description="Claim Borders opaque WebSocket relay")
    parser.add_argument("--host", default=os.environ.get("RELAY_HOST", "127.0.0.1"))
    parser.add_argument("--port", type=int, default=int(os.environ.get("RELAY_PORT", "8765")))
    args = parser.parse_args()
    logging.basicConfig(level=logging.WARNING)
    # Library handshake errors can include request headers; no connection logging.
    logging.getLogger("websockets.server").disabled = True
    asyncio.run(run(args.host, args.port))
