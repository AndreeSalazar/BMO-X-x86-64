"""Ventanas acotadas y vectores ISA sinteticos; no son firmware GA106."""
from contextlib import redirect_stderr, redirect_stdout
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import region_gpu


class Fixture(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.path = Path(tmp.name) / "code.bin"
        self.path.write_bytes(bytes.fromhex("13051000"))  # addi a0, zero, 1


class RegionTests(Fixture):
    def test_exact_region_and_invalid_bounds(self):
        self.assertEqual(region_gpu.region(self.path, 1, 2), bytes.fromhex("0510"))
        for offset, length in ((-1, 1), (0, 0), (0, 65537), (4, 1), (3, 2)):
            with self.subTest(offset=offset, length=length), self.assertRaises(ValueError):
                region_gpu.region(self.path, offset, length)

    def test_isa_alignment_and_address_overflow_are_rejected(self):
        for isa, address, data in (("falcon", 0, b"1234"), ("rv32", 2, b"1234"),
                                   ("rv32", 1 << 32, b"1234"), ("rv64", -4, b"1234"),
                                   ("rv64", (1 << 64) - 4, b"12345678")):
            with self.subTest(isa=isa, address=address), self.assertRaises(ValueError):
                region_gpu.decode(data, isa, address)

    def test_bad_region_cli_has_no_json(self):
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            status = region_gpu.main([str(self.path), "--isa", "rv64", "--offset", "4",
                                 "--length", "1", "--address", "0"])
        self.assertEqual(status, 2)
        self.assertEqual(out.getvalue(), "")

    def test_missing_optional_decoder_is_an_explained_error(self):
        with patch.dict("sys.modules", {"capstone": None}):
            with self.assertRaisesRegex(ValueError, "falta Capstone"):
                region_gpu.decode(self.path.read_bytes(), "rv64", 0)


@unittest.skipUnless(importlib.util.find_spec("capstone"), "requiere requirements-host.txt")
class DecoderTests(Fixture):
    def test_real_engine_rv32_and_rv64_preserve_bytes_and_address(self):
        data = self.path.read_bytes()
        for isa in ("rv32", "rv64"):
            result = region_gpu.decode(data, isa, 0x1000)
            self.assertEqual(result["stop"], "end")
            self.assertEqual(result["consumed"], 4)
            self.assertEqual(result["region_sha256"], hashlib.sha256(data).hexdigest())
            self.assertEqual(result["instructions"][0]["address"], 0x1000)
            self.assertEqual(result["instructions"][0]["bytes"], "13051000")
            self.assertEqual(result["instructions"][0]["mnemonic"], "addi")
            self.assertEqual(result["instructions"][0]["operands"], "a0, zero, 1")

    def test_compressed_is_explicit(self):
        data = bytes.fromhex("0100")  # c.nop
        self.assertEqual(region_gpu.decode(data, "rv64", 2, True)["consumed"], 2)
        self.assertEqual(region_gpu.decode(data, "rv64", 0)["consumed"], 0)

    def test_unknown_and_truncated_stop_without_skipdata(self):
        for tail in (b"\xff" * 4, b"\x13"):
            result = region_gpu.decode(self.path.read_bytes() + tail, "rv64", 0)
            self.assertEqual(result["consumed"], 4)
            self.assertEqual(result["undecoded_hex"], tail.hex())
            self.assertEqual(result["stop"], "unrecognized_or_truncated")

    def test_cli_json_roundtrip(self):
        out = io.StringIO()
        with redirect_stdout(out):
            status = region_gpu.main([str(self.path), "--isa", "rv64", "--offset", "0",
                                 "--length", "4", "--address", "0x1000"])
        self.assertEqual(status, 0)
        result = json.loads(out.getvalue())
        self.assertEqual(result["file_offset"], 0)
        self.assertEqual(result["decoder"]["name"], "capstone")


if __name__ == "__main__":
    unittest.main()
