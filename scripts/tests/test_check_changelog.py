#!/usr/bin/env python3
"""Tests of scripts/check_changelog.py (stdlib only): python3 -m unittest discover -s scripts/tests -v"""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "check_changelog.py"

HEAD = "# История изменений\n\nКраткое вступление.\n\n"
UNRELEASED_FULL = "## [Unreleased]\n\n### Добавлено\n\n- Новая команда.\n\n"
UNRELEASED_EMPTY = "## [Unreleased]\n\n"
V020 = "## [0.2.0] - 2026-11-05\n\n### Изменено\n\n- Изменение.\n\n"
V010 = "## [0.1.0] - 2026-10-29\n\n### Добавлено\n\n- Первый выпуск.\n\n### Исправлено\n\n- Ошибка.\n"


def run(*args):
    r = subprocess.run([sys.executable, str(SCRIPT), *args], capture_output=True, encoding="utf-8")
    return r.returncode, r.stdout, r.stderr


class Base(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / "CHANGELOG.md"
        self.cargo = Path(self.tmp.name) / "Cargo.toml"
        self.set_cargo("0.2.0")

    def write(self, text):
        self.path.write_bytes(text.encode("utf-8"))

    def read(self):
        return self.path.read_bytes().decode("utf-8")

    def set_cargo(self, version):
        self.cargo.write_text(f'[package]\nname = "x"\nversion = "{version}"\n', encoding="utf-8")

    def cl(self, *args):
        return run(*args, "--file", str(self.path), "--cargo", str(self.cargo))


class CheckTests(Base):
    def test_valid_file_passes(self):
        self.write(HEAD + UNRELEASED_FULL + V020 + V010)
        code, out, err = self.cl("check")
        self.assertEqual(code, 0, err)

    def test_default_command_is_check(self):
        self.write(HEAD + UNRELEASED_EMPTY + V010)
        self.assertEqual(self.cl()[0], 0)

    def test_empty_unreleased_is_valid(self):
        self.write(HEAD + UNRELEASED_EMPTY + V010)
        self.assertEqual(self.cl("check")[0], 0)

    def test_prerelease_versions(self):
        self.write(HEAD + UNRELEASED_EMPTY + "## [0.2.0] - 2026-11-05\n\n### Добавлено\n\n- a\n\n"
                   "## [0.2.0-rc.1] - 2026-11-01\n\n### Добавлено\n\n- b\n\n" + V010)
        self.assertEqual(self.cl("check")[0], 0)

    def test_file_missing(self):
        code, _, err = self.cl("check")
        self.assertEqual(code, 1)
        self.assertIn("нет файла", err)

    def test_wrong_order(self):
        self.write(HEAD + UNRELEASED_FULL + V010.replace("2026-10-29", "2026-11-05") + "\n" +
                   V020.replace("2026-11-05", "2026-10-29"))
        code, _, err = self.cl("check")
        self.assertEqual(code, 1)
        self.assertIn("убывать", err)

    def test_bad_date(self):
        self.write(HEAD + UNRELEASED_FULL + V010.replace("2026-10-29", "2026-13-40"))
        code, _, err = self.cl("check")
        self.assertEqual(code, 1)
        self.assertIn("CHANGELOG.md:", err)
        self.assertIn("дата", err)

    def test_date_format(self):
        self.write(HEAD + UNRELEASED_FULL + V010.replace("2026-10-29", "29.10.2026"))
        self.assertEqual(self.cl("check")[0], 1)

    def test_date_grows_downwards(self):
        self.write(HEAD + UNRELEASED_FULL + V020 + V010.replace("2026-10-29", "2026-12-01"))
        code, _, err = self.cl("check")
        self.assertEqual(code, 1)
        self.assertIn("даты", err)

    def test_empty_released_section(self):
        self.write(HEAD + UNRELEASED_FULL + "## [0.2.0] - 2026-11-05\n\n" + V010)
        code, _, err = self.cl("check")
        self.assertEqual(code, 1)
        self.assertIn("нет ни одного пункта", err)

    def test_unknown_subsection(self):
        self.write(HEAD + UNRELEASED_FULL + V010.replace("### Добавлено", "### Новое"))
        code, _, err = self.cl("check")
        self.assertEqual(code, 1)
        self.assertIn("неизвестный подраздел", err)

    def test_empty_subsection_heading(self):
        self.write(HEAD + UNRELEASED_FULL + V010 + "\n### Удалено\n")
        code, _, err = self.cl("check")
        self.assertEqual(code, 1)
        self.assertIn("пуст", err)

    def test_duplicate_version(self):
        self.write(HEAD + UNRELEASED_FULL + V010 + "\n" + V010)
        code, _, err = self.cl("check")
        self.assertEqual(code, 1)
        self.assertIn("уже есть", err)

    def test_missing_unreleased(self):
        self.write(HEAD + V010)
        code, _, err = self.cl("check")
        self.assertEqual(code, 1)
        self.assertIn("[Unreleased]", err)

    def test_unreleased_not_first(self):
        self.write(HEAD + V010 + "\n" + UNRELEASED_FULL)
        self.assertEqual(self.cl("check")[0], 1)

    def test_bad_version_heading(self):
        self.write(HEAD + UNRELEASED_FULL + V010.replace("[0.1.0]", "[0.1]"))
        self.assertEqual(self.cl("check")[0], 1)

    def test_bullet_must_start_with_dash(self):
        self.write(HEAD + UNRELEASED_FULL + V010.replace("- Первый выпуск.", "* Первый выпуск."))
        code, _, err = self.cl("check")
        self.assertEqual(code, 1)
        self.assertIn("`- `", err)

    def test_error_has_line_number(self):
        self.write(HEAD + UNRELEASED_FULL + V010.replace("### Исправлено", "### Прочее"))
        code, _, err = self.cl("check")
        self.assertEqual(code, 1)
        line = [i for i, l in enumerate(self.read().splitlines(), 1) if "Прочее" in l][0]
        self.assertIn(f"CHANGELOG.md:{line}:", err)


class PrepareTests(Base):
    def test_happy_path(self):
        self.write(HEAD + UNRELEASED_FULL + V010)
        code, out, err = self.cl("prepare", "0.2.0", "--date", "2026-11-05")
        self.assertEqual(code, 0, err)
        text = self.read()
        expected = HEAD + UNRELEASED_EMPTY + "## [0.2.0] - 2026-11-05\n\n### Добавлено\n\n- Новая команда.\n\n" + V010
        self.assertEqual(text, expected)
        self.assertNotIn("\r", text)
        self.assertEqual(self.cl("check")[0], 0)
        self.assertEqual(self.cl("release", "0.2.0")[0], 0)

    def test_default_date_is_today(self):
        import datetime
        self.write(HEAD + UNRELEASED_FULL)
        self.assertEqual(self.cl("prepare", "0.1.0")[0], 0)
        self.assertIn(f"## [0.1.0] - {datetime.date.today().isoformat()}", self.read())

    def test_dry_run_changes_nothing(self):
        text = HEAD + UNRELEASED_FULL + V010
        self.write(text)
        code, out, _ = self.cl("prepare", "0.2.0", "--date", "2026-11-05", "--dry-run")
        self.assertEqual(code, 0)
        self.assertIn("Новая команда", out)
        self.assertEqual(self.read(), text)

    def test_refuses_empty_unreleased(self):
        text = HEAD + UNRELEASED_EMPTY + V010
        self.write(text)
        code, _, err = self.cl("prepare", "0.2.0", "--date", "2026-11-05")
        self.assertEqual(code, 1)
        self.assertIn("в [Unreleased] нет записей: описывать нечего, релиз запрещён", err)
        self.assertEqual(self.read(), text)

    def test_refuses_not_greater_version(self):
        text = HEAD + UNRELEASED_FULL + V020 + V010
        self.write(text)
        for version in ("0.2.0", "0.1.5", "0.2.0-rc.1"):
            code, _, err = self.cl("prepare", version, "--date", "2026-11-06")
            self.assertEqual(code, 1, version)
        self.assertEqual(self.read(), text)

    def test_refuses_existing_version(self):
        self.write(HEAD + UNRELEASED_FULL + V010)
        code, _, err = self.cl("prepare", "0.1.0", "--date", "2026-11-06")
        self.assertEqual(code, 1)
        self.assertIn("уже есть", err)

    def test_refuses_date_before_previous(self):
        text = HEAD + UNRELEASED_FULL + V010
        self.write(text)
        code, _, err = self.cl("prepare", "0.2.0", "--date", "2026-01-01")
        self.assertEqual(code, 1)
        self.assertEqual(self.read(), text)

    def test_bad_version_and_date_arguments(self):
        self.write(HEAD + UNRELEASED_FULL)
        self.assertEqual(self.cl("prepare", "1.0")[0], 2)
        self.assertEqual(self.cl("prepare", "1.0.0", "--date", "2026-02-30")[0], 2)


class ReleaseTests(Base):
    def test_ok(self):
        self.write(HEAD + UNRELEASED_EMPTY + V020 + V010)
        code, _, err = self.cl("release", "0.2.0")
        self.assertEqual(code, 0, err)

    def test_missing_entry(self):
        self.write(HEAD + UNRELEASED_EMPTY + V010)
        code, _, err = self.cl("release", "0.2.0")
        self.assertEqual(code, 1)
        self.assertIn("нет записи 0.2.0 в CHANGELOG.md", err)

    def test_unreleased_still_has_bullets(self):
        self.write(HEAD + UNRELEASED_FULL + V020 + V010)
        code, _, err = self.cl("release", "0.2.0")
        self.assertEqual(code, 1)
        self.assertIn("[Unreleased]", err)

    def test_cargo_version_mismatch(self):
        self.write(HEAD + UNRELEASED_EMPTY + V020 + V010)
        self.set_cargo("0.1.0")
        code, _, err = self.cl("release", "0.2.0")
        self.assertEqual(code, 1)
        self.assertIn("Cargo.toml", err)

    def test_not_the_newest_section(self):
        self.write(HEAD + UNRELEASED_EMPTY + V020 + V010)
        self.set_cargo("0.1.0")
        code, _, err = self.cl("release", "0.1.0")
        self.assertEqual(code, 1)
        self.assertIn("не самый новый", err)

    def test_invalid_file_fails(self):
        self.write(HEAD + UNRELEASED_EMPTY + "## [0.2.0] - 2026-11-05\n\n" + V010)
        self.assertEqual(self.cl("release", "0.2.0")[0], 1)


class NotesTests(Base):
    def test_notes_body_without_heading(self):
        self.write(HEAD + UNRELEASED_FULL + V020 + V010)
        code, out, err = self.cl("notes", "0.2.0")
        self.assertEqual(code, 0, err)
        self.assertEqual(out, "### Изменено\n\n- Изменение.\n")

    def test_notes_oldest_section_to_end_of_file(self):
        self.write(HEAD + UNRELEASED_FULL + V020 + V010)
        code, out, _ = self.cl("notes", "0.1.0")
        self.assertEqual(code, 0)
        self.assertEqual(out, "### Добавлено\n\n- Первый выпуск.\n\n### Исправлено\n\n- Ошибка.\n")

    def test_notes_missing(self):
        self.write(HEAD + UNRELEASED_FULL + V010)
        code, _, err = self.cl("notes", "0.3.0")
        self.assertEqual(code, 1)
        self.assertIn("нет записи 0.3.0 в CHANGELOG.md", err)


class RealFileTests(unittest.TestCase):
    def test_repository_changelog_is_valid(self):
        code, _, err = run("check")
        self.assertEqual(code, 0, err)


if __name__ == "__main__":
    unittest.main()
