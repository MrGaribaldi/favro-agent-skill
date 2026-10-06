"""Opt-in dependency regression using only newly created scratch cards.

Requires FAVRO_TEST_COLLECTION and FAVRO_TEST_BOARD; see README.md. Archives
every created card on completion, including failure. Standard library only.
"""

import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
import uuid


class LiveDependencies(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.env = dict(os.environ)
        env_file = Path(cls.env.get("FAVRO_ENV_FILE", "favro.env")).resolve()
        if env_file.is_file():
            for line in env_file.read_text().splitlines():
                if "=" in line and not line.lstrip().startswith("#"):
                    key, value = line.split("=", 1)
                    if key.strip().startswith("FAVRO_"):
                        cls.env.setdefault(key.strip(), value.strip().strip('"').strip("'"))
        required = ("FAVRO_EMAIL", "FAVRO_TOKEN", "FAVRO_TEST_COLLECTION", "FAVRO_TEST_BOARD")
        if any(not cls.env.get(key) for key in required):
            raise unittest.SkipTest("Requires credentials and explicit scratch collection/board")
        cls.binary = str(Path(cls.env.get(
            "FAVRO_TEST_BINARY", Path(__file__).resolve().parents[1] / "target/debug/favro"
        )).resolve())
        cls.collection = cls.env["FAVRO_TEST_COLLECTION"]
        cls.workspace = tempfile.TemporaryDirectory(prefix="favro-dependencies-")
        cls.addClassCleanup(cls.workspace.cleanup)
        for key in ("FAVRO_PROJECT_CONFIG", "FAVRO_ROLE", "FAVRO_ENV_FILE"):
            cls.env.pop(key, None)
        cls.env["FAVRO_STATE"] = str(Path(cls.workspace.name) / "state.json")

    def invoke(self, *args):
        return subprocess.run(
            [self.binary, "--collection", self.collection, *args],
            cwd=self.workspace.name, env=self.env, text=True,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120,
        )

    def cli(self, *args):
        result = self.invoke(*args)
        self.assertEqual(result.returncode, 0, f"{args[0]} failed: {result.stderr}")
        return result.stdout

    def refuse(self, *args):
        result = self.invoke(*args)
        self.assertNotEqual(result.returncode, 0, f"Unexpected success: {result.stdout}")
        self.assertIn("No dependency to remove", result.stderr)
        self.assertNotIn("Dependency removed:", result.stdout)

    def new_card(self, label):
        output = self.cli("add", "--board", self.env["FAVRO_TEST_BOARD"],
                          "--title", f"Dependency regression {label} {uuid.uuid4().hex}")
        match = re.search(r"\[([^\[\]]+)\]\s*$", output)
        self.assertIsNotNone(match, "Created card ID missing from CLI output")
        common_id = match.group(1)
        # Register cleanup before any further request can fail.
        self.addCleanup(self.cli, "archive", "--card", common_id)
        return common_id, json.loads(self.cli("get", "--card", common_id))

    def test_add_list_remove_confirm_and_refuse_second_removal(self):
        a, a_card = self.new_card("A")
        b, b_card = self.new_card("B")
        for before in (False, True):
            with self.subTest(before=before):
                # Exercise common IDs, #number and bare-number references in both
                # argument positions while resolving the same scratch instances.
                card = f"#{a_card['sequentialId']}" if before else a
                on = b if before else str(b_card["sequentialId"])
                direction = ("--before",) if before else ()
                args = ("depend", "--card", card, "--on", on)
                added = self.cli(*args, *direction)
                self.assertIn("Dependency set:", added)
                deps = json.loads(self.cli("deps", "--card", a, "--json"))
                self.assertEqual(len(deps), 1)
                self.assertEqual(deps[0]["cardId"], b_card["cardId"])
                self.assertEqual(deps[0]["isBefore"], not before)
                readable = self.cli("deps", "--card", card)
                self.assertIn("blocks" if before else "depends on", readable)

                # A removal in the other direction must leave the link intact.
                wrong_direction = () if before else ("--before",)
                self.refuse(*args, *wrong_direction, "--remove")
                self.assertEqual(deps, json.loads(self.cli("deps", "--card", a, "--json")))

                # Switch reference forms for removal, covering both --card and --on.
                remove_card = a if before else str(a_card["sequentialId"])
                remove_on = b if before else f"#{b_card['sequentialId']}"
                remove_args = ("depend", "--card", remove_card,
                               "--on", remove_on, *direction, "--remove")
                removed = self.cli(*remove_args)
                relationship = (f"{remove_card} must finish before {remove_on}." if before else
                                f"{remove_card} depends on {remove_on} ({remove_on} must finish first).")
                self.assertEqual(removed, f"Dependency removed: {relationship}\n")
                self.assertEqual([], json.loads(self.cli("deps", "--card", a, "--json")))
                self.assertIn("No dependencies", self.cli("deps", "--card", a))
                self.refuse(*remove_args)


if __name__ == "__main__":
    unittest.main(verbosity=2)
