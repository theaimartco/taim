"""Exercise the real release runner with isolated tool stubs, without building."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ReleaseRunnerTests(unittest.TestCase):
    def run_check(self, options=(), *, dirty=False, fail=False, change=False):
        with tempfile.TemporaryDirectory(prefix="taim-release-runner-") as directory:
            scratch = Path(directory)
            tools = scratch / "bin"
            tools.mkdir()
            calls = scratch / "calls.txt"
            stub = "#!" + sys.executable + "\n" + textwrap.dedent('''\
                import json, os
                from pathlib import Path
                import sys
                name = Path(sys.argv[0]).name
                args = sys.argv[1:]
                calls = Path(os.environ["TAIM_TEST_CALLS"])
                with calls.open("a") as out:
                    out.write(json.dumps([name, *args]) + "\\n")
                if name == "git":
                    if "rev-parse" in args:
                        print("a" * 40)
                    elif "status" in args:
                        if os.environ.get("TAIM_TEST_DIRTY") == "1":
                            print("?? untracked.rs")
                        elif os.environ.get("TAIM_TEST_CHANGE") == "1" and "cargo" in calls.read_text():
                            print(" M src/lib.rs")
                elif name == "cargo":
                    if args[0] == "metadata":
                        print(json.dumps({"packages": [{"name": "fixture"}]}))
                    elif args[0] == "run":
                        Path(args[args.index("--") + 1]).mkdir(parents=True)
                    elif args[0] == "clippy" and os.environ.get("TAIM_TEST_FAIL") == "1":
                        sys.exit(1)
                elif name == "python3":
                    if args[0] == "-c":
                        os.execv(sys.executable, [sys.executable, *args])
            ''')
            for tool in ("git", "cargo", "python3", "cargo-deny"):
                path = tools / tool
                path.write_text(stub)
                path.chmod(0o755)
            env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                       TAIM_QA_PYTHON=str(tools / "python3"),
                       TAIM_TEST_CALLS=str(calls), TAIM_TEST_DIRTY=str(int(dirty)),
                       TAIM_TEST_FAIL=str(int(fail)), TAIM_TEST_CHANGE=str(int(change)))
            evidence = scratch / "evidence"
            result = subprocess.run(["bash", str(ROOT / "scripts/release/check.sh"), *options, str(evidence)],
                                    cwd=ROOT, env=env, capture_output=True, text=True, timeout=30)
            report = (evidence / "report.md").read_text() if (evidence / "report.md").exists() else ""
            return result, report, calls.read_text() if calls.exists() else ""

    def test_untracked_source_blocks_release_before_building(self):
        result, _, calls = self.run_check(dirty=True)
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("clean checkout", result.stderr)
        self.assertNotIn('"cargo"', calls)

    def test_dirty_diagnostics_never_pass_the_release_gate(self):
        result, report, _ = self.run_check(("--allow-dirty", "--host-only"), dirty=True)
        self.assertEqual(result.returncode, 3, result.stdout + result.stderr)
        self.assertIn("Clean tree: NO", report)
        self.assertIn("dirty checkout", report)

    def test_host_only_checks_are_incomplete_and_commands_use_the_lockfile(self):
        import json

        result, report, calls = self.run_check(("--host-only",))
        self.assertEqual(result.returncode, 3, result.stdout + result.stderr)
        self.assertIn("Scope: host-only", report)
        self.assertIn("Log SHA-256 checksums", report)
        commands = [json.loads(line) for line in calls.splitlines()]
        locked = [args for name, *args in commands
                  if name == "cargo" and (args[0] in ("metadata", "test", "run")
                                          or args[:3] == ["clippy", "-p", "fixture"]
                                          or "--workspace" in args)]
        self.assertGreaterEqual(len(locked), 4)
        self.assertTrue(all("--locked" in args for args in locked), locked)

    def test_failed_lints_stop_later_steps(self):
        result, report, calls = self.run_check(("--host-only",), fail=True)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("FAIL | desktop lints", report)
        self.assertNotIn('"cargo", "test"', calls)

    def test_source_changes_invalidate_a_successful_run(self):
        result, report, _ = self.run_check(("--host-only",), change=True)
        self.assertEqual(result.returncode, 3, result.stdout + result.stderr)
        self.assertIn("source consistency", report)
        self.assertIn("checkout changed", report)


if __name__ == "__main__":
    unittest.main()
