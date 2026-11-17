"""SDK identity, replacement and source compatibility are separate prerequisites."""
import copy
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "verify_sdk", Path(__file__).resolve().parents[1] / "verify_sdk.py"
)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class SdkContractTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.file = "crates/sunrise-client/src/net.rs"
        self.path = self.root / self.file
        self.path.parent.mkdir(parents=True)
        self.body = b"fn brand(_address: &str) -> Vec<u8> { Vec::new() }"
        self.manifest = {"native": {
            "sunrise": "a" * 40, "mixins": "b" * 40,
            "mixin": [{"target": checker.BRAND}],
        }}
        self.sdk = {"schema": 1, "build": {"commit": "a" * 40},
                    "mixins": {"rev": "b" * 40}}
        self.functions = {"schema": 1, "build": {"commit": "a" * 40},
                          "functions": {}, "refused": {}, "crates": {}}
        self.set_source(self.body)

    def set_source(self, body):
        # Non-ASCII prefix catches accidental use of character offsets for byte spans.
        prefix = "// Контракт\n".encode()
        data = prefix + body + b"\n"
        self.path.write_bytes(data)
        self.functions["functions"][checker.BRAND] = {
            "module": "sunrise_client_lib::net", "file": self.file,
            "span": [len(prefix), len(prefix) + len(body)],
            "sha256": hashlib.sha256(body).hexdigest(),
        }
        self.functions["crates"] = {"sunrise_client_lib": {"modules": {
            "sunrise_client_lib::net": [{
                "path": self.file, "sha256": hashlib.sha256(data).hexdigest(),
            }],
        }}}

    def report(self):
        return checker.verify(self.manifest, self.sdk, self.functions, self.root)

    def codes(self):
        return {entry["code"] for entry in self.report()["errors"]}

    def test_matching_hook_passes_prerequisites_without_claiming_runtime(self):
        report = self.report()
        self.assertTrue(report["ok"])
        self.assertFalse(report["runtime_verified"])

    def test_source_function_alone_does_not_make_it_replaceable(self):
        self.functions["functions"].clear()
        self.assertEqual(self.codes(), {"SDK_TARGET_UNAVAILABLE"})

    def test_sdk_index_and_mixin_pins_must_all_match(self):
        for document, key in ((self.sdk, "build"), (self.functions, "build")):
            original = copy.deepcopy(document[key])
            document[key]["commit"] = "c" * 40
            self.assertIn("SDK_BUILD", self.codes())
            document[key] = original
        self.sdk["mixins"]["rev"] = "c" * 40
        self.assertIn("SDK_MIXINS", self.codes())

    def test_old_no_address_signature_is_rejected_even_when_indexed(self):
        self.set_source(b"fn brand() -> Vec<u8> { Vec::new() }")
        self.assertEqual(self.codes(), {"SDK_SIGNATURE"})

    def test_different_owner_with_the_same_name_is_not_accepted(self):
        entry = self.functions["functions"].pop(checker.BRAND)
        self.functions["functions"]["sunrise_client::net::brand"] = entry
        self.assertEqual(self.codes(), {"SDK_TARGET_UNAVAILABLE"})

    def test_stale_source_and_invalid_byte_spans_are_rejected(self):
        self.path.write_bytes(self.path.read_bytes() + b"// changed\n")
        self.assertEqual(self.codes(), {"SDK_SOURCE_HASH"})
        self.set_source(self.body)
        self.functions["functions"][checker.BRAND]["span"] = [-1, 3]
        self.assertEqual(self.codes(), {"SDK_SOURCE_SPAN"})
        self.functions["functions"][checker.BRAND]["span"] = [0, 3]
        self.assertEqual(self.codes(), {"SDK_FUNCTION_HASH"})

    def test_index_cannot_read_outside_the_checkout(self):
        self.functions["functions"][checker.BRAND]["file"] = "../outside.rs"
        self.assertEqual(self.codes(), {"SDK_SOURCE_PATH"})

    def test_unknown_schemas_are_rejected(self):
        self.functions["schema"] = 2
        self.assertEqual(self.codes(), {"SDK_SCHEMA"})


if __name__ == "__main__":
    unittest.main()
