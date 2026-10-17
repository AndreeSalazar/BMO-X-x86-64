"""Comparar evidencia declarada no equivale a encontrar una causa."""
from contextlib import redirect_stderr, redirect_stdout
import copy
import io
import json
from pathlib import Path
import tempfile
import unittest

import sesion


class SessionTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.path = Path(tmp.name) / "copy.bin"
        self.path.write_bytes(b"abc")

    def sample(self):
        return sesion.make(["logs=" + str(self.path)], {}, {
            "case": "synthetic", "outcome": "unknown", "boot": "unknown",
            "point": "unknown", "variable": "unknown"})

    def test_missing_identity_is_visible_even_for_equal_files(self):
        data = self.sample()
        result = sesion.compare(data, data)
        self.assertIn("build", result["missing"]["before"])
        self.assertIn("artifact:gsp", result["missing"]["after"])
        self.assertIn("context:boot", result["missing"]["before"])
        self.assertEqual(result["evidence"]["artifacts"]["logs"]["status"], "same_file_digest")
        self.assertNotIn("cause", result)

    def test_good_bad_is_declared_and_does_not_change_measured_hash(self):
        a = self.sample()
        b = copy.deepcopy(a)
        a["context"]["outcome"] = "good"
        b["context"]["outcome"] = "bad"
        result = sesion.compare(a, b)
        self.assertEqual(result["context_changes"]["outcome"], {"before": "good", "after": "bad"})
        self.assertEqual(result["evidence"]["artifacts"]["logs"]["status"], "same_file_digest")

    def test_closed_schema_and_invalid_context_are_rejected(self):
        for key, value in (("outcome", "working"), ("boot", "reboot"),
                           ("case", ""), ("point", 1), ("variable", "x" * 257)):
            data = self.sample()
            data["context"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                sesion.validate(data)
        data = self.sample()
        data["cause"] = "0x15"
        with self.assertRaises(ValueError):
            sesion.validate(data)

    def test_inventory_is_validated_inside_session(self):
        data = self.sample()
        data["inventory"]["artifacts"]["logs"]["sha256"] = "fake"
        with self.assertRaises(ValueError):
            sesion.compare(self.sample(), data)

    def test_cli_roundtrip_and_unknowns(self):
        out = io.StringIO()
        with redirect_stdout(out):
            status = sesion.main(["registrar", "--case", "synthetic", "--artefacto", "logs=" + str(self.path)])
        self.assertEqual(status, 0)
        data = json.loads(out.getvalue())
        self.assertEqual(data["context"]["outcome"], "unknown")
        self.assertNotIn(str(self.path), out.getvalue())
        left = self.path.with_suffix(".json")
        left.write_text(out.getvalue())
        out = io.StringIO()
        with redirect_stdout(out):
            self.assertEqual(sesion.main(["comparar", str(left), str(left)]), 0)
        self.assertEqual(json.loads(out.getvalue())["context_changes"], {})

    def test_duplicate_json_key_fails_without_partial_result(self):
        self.path.write_text('{"schema":"a","schema":"b"}')
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            self.assertEqual(sesion.main(["comparar", str(self.path), str(self.path)]), 2)
        self.assertEqual(out.getvalue(), "")
        self.assertIn("duplicada", err.getvalue())


if __name__ == "__main__":
    unittest.main()
