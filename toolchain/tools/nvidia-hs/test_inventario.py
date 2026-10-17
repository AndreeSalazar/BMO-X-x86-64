"""Huellas, procedencia y rechazos: todo con archivos sinteticos locales."""

from contextlib import redirect_stderr, redirect_stdout
import hashlib
import io
import json
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import inventario as inv


class InventoryTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.copy = self.root / "firmware.bin"
        self.copy.write_bytes(b"abc")

    def sample(self, **observation):
        return inv.inventory(["gsp=" + str(self.copy)], observation)

    def run_cli(self, *args):
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            status = inv.main(args)
        return status, out.getvalue(), err.getvalue()

    def test_known_sha256_and_no_input_change(self):
        before = self.copy.stat()
        data = self.sample()
        self.assertEqual(data["artifacts"]["gsp"], {
            "bytes": 3,
            "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        })
        self.assertEqual(self.copy.read_bytes(), b"abc")
        self.assertEqual(self.copy.stat().st_mtime_ns, before.st_mtime_ns)

    def test_streaming_multiple_blocks(self):
        content = bytes(range(256)) * (inv.BLOCK // 256 + 1)
        self.copy.write_bytes(content)
        self.assertEqual(inv.fingerprint(self.copy), {
            "bytes": len(content), "sha256": hashlib.sha256(content).hexdigest(),
        })

    def test_changed_snapshot_rejected(self):
        before = self.copy.stat()
        after = SimpleNamespace(st_size=before.st_size,
            st_mtime_ns=before.st_mtime_ns + 1, st_ctime_ns=before.st_ctime_ns)
        with patch.object(inv.os, "fstat", side_effect=[before, after]):
            with self.assertRaisesRegex(ValueError, "cambiado"):
                inv.fingerprint(self.copy)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "FIFO no disponible en este anfitrion")
    def test_fifo_rejected_without_waiting_for_writer(self):
        fifo = self.root / "fifo"
        os.mkfifo(fifo)
        with self.assertRaisesRegex(ValueError, "regular"):
            inv.fingerprint(fifo)

    def test_absent_identity_is_null_and_not_hardware_claim(self):
        data = inv.validate(self.sample())
        self.assertEqual(data["observation"], dict.fromkeys(inv.OBS_KEYS))
        self.assertEqual(data["observation_origin"], "user_declared_not_measured_by_this_tool")
        self.assertEqual(data["limits"], inv.LIMITS)

    def test_duplicate_role_and_empty_files_rejected(self):
        declaration = "gsp=" + str(self.copy)
        with self.assertRaisesRegex(ValueError, "duplicado"):
            inv.inventory([declaration, declaration], {})
        self.copy.write_bytes(b"")
        with self.assertRaisesRegex(ValueError, "vacio"):
            self.sample()

    def test_bad_declarations_and_missing_file(self):
        for value in ("gsp", "unknown=x", "gsp=", "=file"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                inv.inventory([value], {})
        status, out, err = self.run_cli("inventario", "--artefacto", "gsp=" + str(self.root / "absent"))
        self.assertEqual(status, 2)
        self.assertEqual(out, "")
        self.assertIn("inventario HS:", err)

    def test_compare_bytes_and_identity_are_separate(self):
        a, b = self.sample(build="one", attempt=1), self.sample(build="two", attempt=2)
        result = inv.compare(a, b)
        self.assertEqual(result["artifacts"]["gsp"]["status"], "same_file_digest")
        self.assertEqual(result["observation_changes"]["build"], {"before": "one", "after": "two"})
        self.copy.write_bytes(b"abd")
        c = self.sample(build="two", attempt=2)
        self.assertEqual(inv.compare(b, c)["artifacts"]["gsp"]["status"], "different_file_digest")

    def test_added_and_removed_artifacts(self):
        a = self.sample()
        b = inv.inventory(["booter=" + str(self.copy)], {})
        changes = inv.compare(a, b)["artifacts"]
        self.assertEqual(changes["gsp"]["status"], "removed")
        self.assertEqual(changes["booter"]["status"], "added")

    def test_malformed_manifests_rejected(self):
        for mutation in (
            lambda d: d.update(schema="future"),
            lambda d: d.update(limits="firmware_is_valid"),
            lambda d: d.update(observation_origin="hardware"),
            lambda d: d["artifacts"]["gsp"].update(bytes=True),
            lambda d: d["artifacts"]["gsp"].update(sha256="x" * 64),
            lambda d: d["observation"].update(attempt=True),
            lambda d: d["observation"].update(attempt=0),
            lambda d: d["observation"].update(pci_id="unknown"),
        ):
            with self.subTest(mutation=mutation):
                data = self.sample()
                mutation(data)
                with self.assertRaises(ValueError):
                    inv.compare(data, self.sample())

    def test_json_duplicate_and_oversized_rejected(self):
        file = self.root / "bad.json"
        file.write_text('{"schema":"one","schema":"two"}')
        with self.assertRaisesRegex(ValueError, "duplicada"):
            inv.load(file)
        file.write_bytes(b" " * (inv.BLOCK + 1))
        with self.assertRaisesRegex(ValueError, "1 MiB"):
            inv.load(file)

    def test_cli_round_trip_and_no_absolute_paths_in_manifest(self):
        status, out, err = self.run_cli(
            "inventario", "--artefacto", "gsp=" + str(self.copy),
            "--build", "example", "--attempt", "1", "--pci-id", "10de:2504",
        )
        self.assertEqual((status, err), (0, ""))
        self.assertNotIn(str(self.root), out)
        a, b = self.root / "a.json", self.root / "b.json"
        a.write_text(out)
        b.write_text(out)
        status, out, err = self.run_cli("comparar", str(a), str(b))
        self.assertEqual((status, err), (0, ""))
        self.assertEqual(json.loads(out)["observation_changes"], {})


if __name__ == "__main__":
    unittest.main()
