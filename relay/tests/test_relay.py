"""Real loopback WebSocket integration tests; no external services."""
import asyncio
import hashlib
import json
from dataclasses import replace
import unittest
from unittest.mock import patch
from urllib.request import urlopen

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey
from websockets.asyncio.client import connect
from websockets.exceptions import ConnectionClosed, InvalidStatus

from dhrelay.server import AUTH_PREFIX, Limits, Relay, encode


class Identity:
    def __init__(self, realm="test"):
        realm = hashlib.sha256(realm.encode()).hexdigest()
        self.key = Ed25519PrivateKey.generate()
        public = self.key.public_key().public_bytes_raw()
        self.code = hashlib.sha256(public).digest()[:12].hex()
        self.cert = encode(["id1", realm, public.hex(),
                           X25519PrivateKey.generate().public_key().public_bytes_raw().hex(),
                           "00" * 16, "Player"])
        self.signature = self.key.sign(self.cert.encode()).hex()

    def auth(self, challenge):
        proof = self.key.sign(AUTH_PREFIX + bytes.fromhex(challenge[1]) + self.cert.encode()).hex()
        return ["auth", self.cert, self.signature, proof]


class RelayTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.relay = Relay()
        self.server = await self.relay.serve(port=0)
        self.port = self.server.sockets[0].getsockname()[1]
        self.url = f"ws://127.0.0.1:{self.port}/v1/relay"
        self.sockets = []

    async def asyncTearDown(self):
        for ws in self.sockets:
            await ws.close()
        self.server.close()
        await self.server.wait_closed()

    async def socket(self):
        ws = await connect(self.url, compression=None, max_queue=256, close_timeout=1)
        self.sockets.append(ws)
        return ws

    async def receive(self, ws):
        return json.loads(await asyncio.wait_for(ws.recv(), 1))

    async def login(self, identity=None):
        identity = identity or Identity()
        ws = await self.socket()
        await ws.send(encode(identity.auth(await self.receive(ws))))
        self.assertEqual(await self.receive(ws), ["ready", identity.code])
        return ws, identity

    async def test_default_has_no_connection_or_rate_quotas(self):
        clients = [await self.login(Identity()) for _ in range(12)]
        self.assertEqual(self.relay.connections, 12)
        sender, _ = clients[0]
        receiver, identity = clients[1]
        for n in range(40):
            await sender.send(encode(["box", identity.code, str(n)]))
            while True:
                frame = await self.receive(receiver)
                if frame[0] == "box":
                    self.assertEqual(frame[2], str(n))
                    break

    async def test_initial_directory_can_exceed_live_queue_size(self):
        self.relay.limits = replace(Limits(), queue=2)
        for _ in range(3):
            await self.login(Identity())
            await asyncio.sleep(0.01)
        ws, _ = await self.login(Identity())
        frames = [await self.receive(ws) for _ in range(3)]
        self.assertTrue(all(frame[0] == "peer" for frame in frames))

    async def test_routing_realm_isolation_and_cleanup(self):
        alice, a = await self.login()
        bob, b = await self.login()
        self.assertEqual((await self.receive(alice))[1], b.cert)
        self.assertEqual((await self.receive(bob))[1], a.cert)
        eve, e = await self.login(Identity("other-realm"))
        opaque = "opaque encrypted payload: not even valid base64"
        await alice.send(encode(["box", b.code, opaque]))
        self.assertEqual(await self.receive(bob), ["box", a.code, opaque])
        await eve.send(encode(["box", b.code, "isolated"]))
        with self.assertRaises(asyncio.TimeoutError):
            await asyncio.wait_for(bob.recv(), 0.05)
        await bob.close()
        self.assertEqual(await self.receive(alice), ["gone", b.code])
        self.assertNotIn(b.key.public_key().public_bytes_raw(), self.relay.peers)
        await alice.send(encode(["box", b.code, "offline drops"]))
        self.assertEqual(await self.receive(alice), ["error", "recipient_offline"])
        self.assertEqual(await self.receive(eve), ["error", "recipient_offline"])
        self.assertEqual(len(self.relay.codes), 2)

    async def test_forgery_replay_and_duplicate_identity(self):
        ident = Identity()
        first = await self.socket()
        saved = ident.auth(await self.receive(first))
        await first.send(encode(saved))
        self.assertEqual((await self.receive(first))[0], "ready")
        replay = await self.socket()
        await self.receive(replay)
        await replay.send(encode(saved))
        with self.assertRaises(ConnectionClosed):
            await replay.recv()
        forged = await self.socket()
        frame = ident.auth(await self.receive(forged))
        frame[2] = "00" * 64
        await forged.send(encode(frame))
        with self.assertRaises(ConnectionClosed):
            await forged.recv()
        duplicate = await self.socket()
        await duplicate.send(encode(ident.auth(await self.receive(duplicate))))
        with self.assertRaises(ConnectionClosed):
            await duplicate.recv()
        self.assertEqual(len(self.relay.peers), 1)
        self.assertIn(ident.key.public_key().public_bytes_raw(), self.relay.peers)
        await first.send(encode(saved))  # Auth frames cannot be replayed after login.
        with self.assertRaises(ConnectionClosed):
            await first.recv()

    async def test_limits_and_malformed_packets(self):
        self.relay.limits = replace(Limits(), rate=0.001, burst=2, ip=2)
        first, ident = await self.login()
        second, _ = await self.login(Identity("other"))
        third = await self.socket()
        with self.assertRaises(ConnectionClosed):
            await third.recv()
        for _ in range(3):
            await first.send(encode(["box", "ff" * 12, "opaque"]))
        with self.assertRaises(ConnectionClosed):
            await first.recv()
        await second.send(encode(["box", "ff" * 12, {"not": "opaque string"}]))
        with self.assertRaises(ConnectionClosed):
            await second.recv()

    async def test_global_and_realm_limits(self):
        self.relay.limits = replace(Limits(), total=2, realm=1)
        alice, _ = await self.login()
        rejected = await self.socket()
        ident = Identity()
        await rejected.send(encode(ident.auth(await self.receive(rejected))))
        with self.assertRaises(ConnectionClosed):
            await rejected.recv()
        bob, _ = await self.login(Identity("other"))
        rejected = await self.socket()
        with self.assertRaises(ConnectionClosed):
            await rejected.recv()
        await bob.close()
        await alice.close()

    async def test_proxy_header_trust_is_explicit(self):
        self.relay.trusted_proxies = ()
        ignored = await connect(self.url, additional_headers={"X-Real-IP": "203.0.113.11"})
        self.sockets.append(ignored)
        await self.receive(ignored)
        self.assertEqual(self.relay.ips["127.0.0.1"], 1)
        self.assertNotIn("203.0.113.11", self.relay.ips)
        await ignored.close()
        import ipaddress
        self.relay.trusted_proxies = (ipaddress.ip_network("127.0.0.1/32"),)
        trusted = await connect(self.url, additional_headers={"X-Real-IP": "203.0.113.12"})
        self.sockets.append(trusted)
        await self.receive(trusted)
        self.assertEqual(self.relay.ips["203.0.113.12"], 1)
        forged_forwarded = await connect(self.url, additional_headers={"X-Forwarded-For": "203.0.113.13"})
        self.sockets.append(forged_forwarded)
        await self.receive(forged_forwarded)
        self.assertNotIn("203.0.113.13", self.relay.ips)

    async def test_shared_translation_requires_private_relay_and_bounds_usage(self):
        url = f"ws://127.0.0.1:{self.port}/v1/translate"
        with self.assertRaises(ValueError):
            Relay(deepl_key="test-key:fx")
        with self.assertRaises(InvalidStatus) as unavailable:
            await connect(url)
        self.assertEqual(unavailable.exception.response.status_code, 404)
        self.relay.deepl_key = "test-key:fx"
        self.relay.access_token = "local-test-token"
        with self.assertRaises(InvalidStatus) as unauthorized:
            await connect(url)
        self.assertEqual(unauthorized.exception.response.status_code, 401)
        headers = {"Authorization": "Bearer local-test-token"}
        with patch("dhrelay.server.translate_deepl", return_value="Hello") as upstream:
            async with connect(url, additional_headers=headers) as ws:
                await ws.send(encode(["EN-US", "Привет"]))
                self.assertEqual(json.loads(await ws.recv()), ["ok", "Hello"])
            upstream.assert_called_once_with("test-key:fx", "Привет", "EN-US")
            for request in (["XX", "Привет"], ["EN-US", "а" * 501], ["RU", "/msg player hi"]):
                async with connect(url, additional_headers=headers) as ws:
                    await ws.send(encode(request))
                    self.assertEqual(json.loads(await ws.recv()), ["error"])
            self.relay.translation_tokens = 0
            async with connect(url, additional_headers=headers) as ws:
                await ws.send(encode(["EN-US", "Привет" * 50]))
                self.assertEqual(json.loads(await ws.recv()), ["error"])
            self.assertEqual(upstream.call_count, 1)

    async def test_common_targets_and_explicit_source_are_forwarded(self):
        url = f"ws://127.0.0.1:{self.port}/v1/translate"
        self.relay.deepl_key = "test-key:fx"
        self.relay.access_token = "local-test-token"
        headers = {"Authorization": "Bearer local-test-token"}
        targets = ["EN-US", "RU", "FI", "PL", "DE", "FR", "ES", "IT", "PT-BR", "UK", "NL", "SV", "DA", "CS", "TR", "AR", "HI", "JA", "KO", "ZH-HANS"]
        with patch("dhrelay.server.translate_deepl", return_value="translated") as upstream:
            for target in targets:
                async with connect(url, additional_headers=headers) as ws:
                    await ws.send(encode([target, "hei", "FI"]))
                    self.assertEqual(json.loads(await ws.recv()), ["ok", "translated"])
                upstream.assert_called_with("test-key:fx", "hei", target, "FI")
            for frame in (["PL", "hello", "XX"], ["XX", "hello", "FI"], ["PL", "hello", "FI", "extra"]):
                async with connect(url, additional_headers=headers) as ws:
                    await ws.send(encode(frame))
                    self.assertEqual(json.loads(await ws.recv()), ["error"])
            self.assertEqual(upstream.call_count, len(targets))

    async def test_token_and_health(self):
        self.relay.access_token = "local-test-token"
        with self.assertRaises(InvalidStatus) as rejection:
            await connect(self.url)
        self.assertEqual(rejection.exception.response.status_code, 401)
        ws = await connect(self.url, additional_headers={"Authorization": "Bearer local-test-token"})
        self.sockets.append(ws)
        self.assertEqual((await self.receive(ws))[0], "challenge")
        def health():
            with urlopen(f"http://127.0.0.1:{self.port}/healthz", timeout=2) as response:
                return response.status, response.read()
        status, body = await asyncio.to_thread(health)
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(body), ["ok", "1.3.0"])

    async def test_auth_deadline_oversize_and_binary(self):
        self.relay.limits = replace(Limits(), auth_timeout=0.05)
        ws = await self.socket()
        await self.receive(ws)
        with self.assertRaises(ConnectionClosed):
            await ws.recv()
        ws, _ = await self.login()
        await ws.send(b"[]")
        with self.assertRaises(ConnectionClosed):
            await ws.recv()
        ws, _ = await self.login()
        await ws.send("x" * 16385)
        with self.assertRaises(ConnectionClosed):
            await ws.recv()


if __name__ == "__main__":
    unittest.main()
