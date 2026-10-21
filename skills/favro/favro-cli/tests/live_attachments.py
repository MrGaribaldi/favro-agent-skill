"""Opt-in regression test against existing scratch boards; see README.md.

Uses only the Python standard library. Creates test cards and archives them on
completion, including failure. Never selects or edits pre-existing cards.
"""

import collections
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
import uuid
import urllib.request
from urllib.parse import urlsplit, urlunsplit


class LiveAttachments(unittest.TestCase):
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
        required = ("FAVRO_EMAIL", "FAVRO_TOKEN", "FAVRO_TEST_COLLECTION",
                    "FAVRO_TEST_BOARD", "FAVRO_TEST_MOVE_BOARD")
        if any(not cls.env.get(key) for key in required):
            raise unittest.SkipTest("Requires credentials and explicit scratch collection/boards")
        cls.binary = str(Path(cls.env.get(
            "FAVRO_TEST_BINARY", Path(__file__).resolve().parents[1] / "target/debug/favro"
        )).resolve())
        cls.collection = cls.env["FAVRO_TEST_COLLECTION"]
        # Isolate configuration discovery and state from the invoking project.
        cls.workspace = tempfile.TemporaryDirectory(prefix="favro-attachments-")
        cls.addClassCleanup(cls.workspace.cleanup)
        for key in ("FAVRO_PROJECT_CONFIG", "FAVRO_ROLE", "FAVRO_ENV_FILE"):
            cls.env.pop(key, None)
        cls.env["FAVRO_STATE"] = str(Path(cls.workspace.name) / "state.json")
        cls.fixture = Path(cls.workspace.name) / "evidence.txt"
        cls.fixture.write_text("Favro attachment regression evidence.\n")

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

    def cli_fail(self, *args):
        result = self.invoke(*args)
        self.assertNotEqual(result.returncode, 0, f"{args[0]} unexpectedly succeeded: {result.stdout}")
        return result.stderr

    @staticmethod
    def identity(url):
        parts = urlsplit(url)
        return urlunsplit((parts.scheme, parts.netloc, parts.path, "", ""))

    def card(self, common_id):
        # Mutation checks inspect native task glyphs and compare descriptions
        # without the expiring signed URLs embedded in Markdown file nodes.
        return json.loads(self.cli("get", "--card", common_id,
                                   "--description-format", "plaintext"))

    def test_attachment_survives_each_update(self):
        cases = [
            ("set-desc", "--text", "Replacement description"),
            ("set-notes", "--text", "Notes after upload"),
            ("set-result", "--text", "Evidence is attached"),
            ("set-todo", "--item", "Review the evidence"),
            ("archive",),
            ("move-board", "--board", self.env["FAVRO_TEST_MOVE_BOARD"]),
        ]
        for args in cases:
            with self.subTest(command=args[0]):
                output = self.cli("add", "--board", self.env["FAVRO_TEST_BOARD"],
                                  "--title", f"Attachment regression {uuid.uuid4().hex}")
                match = re.search(r"\[([^\[\]]+)\]\s*$", output)
                self.assertIsNotNone(match, "Created card ID missing from CLI output")
                common_id = match.group(1)
                archived = False
                try:
                    self.cli("attach", "--card", common_id, "--file", str(self.fixture))
                    before = self.card(common_id)
                    self.assertEqual(len(before.get("attachments", [])), 1)
                    self.cli(args[0], "--card", common_id, *args[1:])
                    archived = args[0] == "archive"
                    if archived:
                        # Also exercise unarchiving; get selects active instances.
                        self.cli("archive", "--card", common_id, "--undo")
                        archived = False
                    after = self.card(common_id)
                    expected = collections.Counter(a["name"] for a in before["attachments"])
                    actual = collections.Counter(a["name"] for a in after.get("attachments", []))
                    self.assertFalse(expected - actual, "Attachment disappeared")
                finally:
                    if not archived:
                        self.cli("archive", "--card", common_id)


    def test_repeated_edits_preserve_duplicate_names_and_checkboxes(self):
        output = self.cli("add", "--board", self.env["FAVRO_TEST_BOARD"],
                          "--title", f"Repeated attachment regression {uuid.uuid4().hex}")
        match = re.search(r"\[([^\[\]]+)\]\s*$", output)
        self.assertIsNotNone(match)
        common_id = match.group(1)
        try:
            # Distinct uploads with the same nontrivial filename must both survive.
            for _ in range(2):
                self.cli("attach", "--card", common_id, "--file", str(self.fixture),
                         "--name", "evidence [final] & *review* (blå).txt")
            expected = collections.Counter(a["name"] for a in self.card(common_id)["attachments"])
            self.assertEqual(sum(expected.values()), 2)
            cases = [
                ("set-desc", "--text=- [x] Human completed\n- [ ] Human pending"),
                ("set-notes", "--text", "First note"),
                ("set-notes", "--text", "Revised note"),
                ("set-result", "--text", "Evidence is attached"),
                ("set-todo", "--item", "Review evidence"),
                ("set-todo", "--clear"),
            ]
            for args in cases:
                self.cli(args[0], "--card", common_id, *args[1:])
                after = self.card(common_id)
                self.assertEqual(expected, collections.Counter(a["name"] for a in after["attachments"]))
                self.assertIn("☑ Human completed", after["detailedDescription"])
                self.assertIn("☐ Human pending", after["detailedDescription"])
            # File nodes must remain outside even an unfinished user code block.
            self.cli("set-desc", "--card", common_id, "--text", "```\nUnfinished code")
            after = self.card(common_id)
            self.assertEqual(expected, collections.Counter(a["name"] for a in after["attachments"]))
        finally:
            self.cli("archive", "--card", common_id)

    def test_replace_leaves_exactly_one_attachment_with_new_bytes(self):
        output = self.cli("add", "--board", self.env["FAVRO_TEST_BOARD"],
                          "--title", f"Replace regression {uuid.uuid4().hex}")
        common_id = re.search(r"\[([^\[\]]+)\]\s*$", output).group(1)
        try:
            self.cli("set-notes", "--card", common_id, "--text", "Notes before replace")
            self.cli("attach", "--card", common_id, "--file", str(self.fixture), "--name", "report.md")
            before = self.card(common_id)
            self.assertEqual(len(before["attachments"]), 1)
            old_url = before["attachments"][0]["fileURL"]

            revised = Path(self.workspace.name) / "revised.txt"
            revised.write_text("Revised evidence.\n")
            self.cli("attach", "--card", common_id, "--file", str(revised),
                     "--name", "report.md", "--replace")

            after = self.card(common_id)
            matching = [a for a in after["attachments"] if a["name"] == "report.md"]
            self.assertEqual(len(matching), 1, "replace must leave exactly one attachment with the name")
            self.assertNotEqual(self.identity(matching[0]["fileURL"]), self.identity(old_url), "replace must point at the new upload")
            with urllib.request.urlopen(matching[0]["fileURL"], timeout=30) as response:
                self.assertEqual(response.read(), revised.read_bytes())
            # The rest of the description (the notes block) must survive untouched.
            self.assertIn("Notes before replace", after["detailedDescription"])
        finally:
            self.cli("archive", "--card", common_id)

    def test_replace_with_no_prior_attachment_is_a_plain_attach(self):
        output = self.cli("add", "--board", self.env["FAVRO_TEST_BOARD"],
                          "--title", f"Replace-first regression {uuid.uuid4().hex}")
        common_id = re.search(r"\[([^\[\]]+)\]\s*$", output).group(1)
        try:
            # --replace with nothing to replace behaves like a plain attach.
            self.cli("attach", "--card", common_id, "--file", str(self.fixture),
                     "--name", "first.md", "--replace")
            after = self.card(common_id)
            self.assertEqual(len(after["attachments"]), 1)
        finally:
            self.cli("archive", "--card", common_id)

    def test_detach_removes_one_attachment_and_leaves_the_rest(self):
        output = self.cli("add", "--board", self.env["FAVRO_TEST_BOARD"],
                          "--title", f"Detach regression {uuid.uuid4().hex}")
        common_id = re.search(r"\[([^\[\]]+)\]\s*$", output).group(1)
        try:
            self.cli("set-notes", "--card", common_id, "--text", "Notes survive detach")
            self.cli("attach", "--card", common_id, "--file", str(self.fixture), "--name", "keep.txt")
            self.cli("attach", "--card", common_id, "--file", str(self.fixture), "--name", "drop.txt")
            self.cli("set-desc", "--card", common_id,
                     "--text=- [x] Completed review\n- [ ] Pending review\nNotes survive detach")
            # Ordinary writes deliberately preserve attachments, even without links.
            self.assertEqual(len(self.card(common_id)["attachments"]), 2)
            dropped_url = next(a["fileURL"] for a in self.card(common_id)["attachments"]
                               if a["name"] == "drop.txt")
            self.cli("attachment-delete", "--card", common_id, "--name", "drop.txt")
            # Probe storage separately; card removal need not revoke signed URLs.
            try:
                with urllib.request.urlopen(dropped_url, timeout=30) as response:
                    status = response.status
            except urllib.error.HTTPError as error:
                status = error.code
            print(f"Detached download URL HTTP status: {status}")
            after = self.card(common_id)
            self.assertEqual([a["name"] for a in after["attachments"]], ["keep.txt"])
            self.assertIn("Notes survive detach", after["detailedDescription"])
            self.assertIn("☑ Completed review", after["detailedDescription"])
            self.assertIn("☐ Pending review", after["detailedDescription"])
            # Also remove the last file: no remaining nodes is a valid rewrite.
            self.cli("detach", "--card", common_id, "--name", "keep.txt")
            self.assertEqual(self.card(common_id).get("attachments", []), [])
        finally:
            self.cli("archive", "--card", common_id)

    def test_replace_and_detach_refuse_ambiguous_duplicate_names(self):
        output = self.cli("add", "--board", self.env["FAVRO_TEST_BOARD"],
                          "--title", f"Ambiguous regression {uuid.uuid4().hex}")
        common_id = re.search(r"\[([^\[\]]+)\]\s*$", output).group(1)
        try:
            # Two distinct uploads sharing a filename -- the case that motivated this feature.
            for index in range(2):
                result = self.invoke("attach", "--card", common_id, "--file", str(self.fixture), "--name", "dup.txt")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual("Warning:" in result.stderr, index == 1)
            before = self.card(common_id)
            self.assertEqual(len(before["attachments"]), 2)
            urls = [a["fileURL"] for a in before["attachments"]]
            snapshot = ([(a["name"], self.identity(a["fileURL"])) for a in before["attachments"]], before.get("detailedDescription"))

            self.cli_fail("detach", "--card", common_id, "--name", "dup.txt")
            self.cli_fail("attach", "--card", common_id, "--file", str(self.fixture),
                          "--name", "dup.txt", "--replace")
            # Refusing an ambiguous target must not have mutated the card either way.
            after_refusal = self.card(common_id)
            self.assertEqual(
                snapshot, ([(a["name"], self.identity(a["fileURL"])) for a in after_refusal["attachments"]], after_refusal.get("detailedDescription"))
            )

            # A targeted replacement preserves the other same-named document.
            self.cli("attach", "--card", common_id, "--file", str(self.fixture),
                     "--name", "dup.txt", "--replace", "--replace-url", urls[0])
            replaced = self.card(common_id)["attachments"]
            self.assertEqual(len(replaced), 2)
            replacement_urls = [a["fileURL"] for a in replaced if self.identity(a["fileURL"]) != self.identity(urls[1])]
            self.assertEqual(len(replacement_urls), 1)
            self.assertNotIn(self.identity(urls[0]), [self.identity(a["fileURL"]) for a in replaced])
            # --url disambiguates: exactly the targeted copy goes, the other survives.
            self.cli("detach", "--card", common_id, "--name", "dup.txt", "--url", replacement_urls[0])
            after = self.card(common_id)
            self.assertEqual([self.identity(a["fileURL"]) for a in after["attachments"]], [self.identity(urls[1])])
        finally:
            self.cli("archive", "--card", common_id)


if __name__ == "__main__":
    unittest.main(verbosity=2)
