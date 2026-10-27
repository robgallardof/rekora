"""Real OpenCode + DeepSeek MCP probes; set OPENCODE_BIN and AGENT_HOST_MODULES.

Build Rekora first (cargo build -p rekora-cli). All configuration and SQLite
state live in a disposable Git repository. No model calls or user config edits.
"""
from pathlib import Path
import json
import os
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    binary = Path(os.environ.get("REKORA_BIN", ROOT / "target/debug" / ("rekora.exe" if os.name == "nt" else "rekora")))
    host = os.environ.get("OPENCODE_BIN") or shutil.which("opencode")
    if not host or not os.environ.get("AGENT_HOST_MODULES") or not binary.is_file():
        raise SystemExit("build Rekora; set OPENCODE_BIN and AGENT_HOST_MODULES; NOT EXECUTED")
    with tempfile.TemporaryDirectory(prefix="rekora-hosts-") as folder:
        work = Path(folder)
        env = {**os.environ, "HOME": folder, "USERPROFILE": folder, "APPDATA": str(work / "appdata"),
               "LOCALAPPDATA": str(work / "local"), "XDG_CONFIG_HOME": str(work / "config"),
               "XDG_DATA_HOME": str(work / "data"), "XDG_CACHE_HOME": str(work / "cache"),
               "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": str(work / "gitconfig"),
               "OPENCODE_DISABLE_AUTOUPDATE": "true", "OPENCODE_DISABLE_DEFAULT_PLUGINS": "true"}
        def run(args):
            return subprocess.run(args, cwd=work, env=env, check=True, capture_output=True, text=True,
                                  encoding="utf-8", errors="replace", timeout=90)
        run(["git", "init", "-q"])
        run([str(binary), "init"])
        for client in ("opencode", "deepseek"):
            run([str(binary), "agent", "install", client, "--confirm"])
        listed = run([host, "mcp", "list"])
        plain = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", listed.stdout + listed.stderr)
        assert "rekora" in plain and "connected" in plain.lower(), plain
        print("OpenCode: rekora connected", flush=True)
        overlay = json.loads((work / ".dsh/rekora.cordis.yml").read_text())
        config = work / "probe.json"
        config.write_text(json.dumps({"config": overlay[0]["config"], "tool": "status", "args": {}}))
        subprocess.run(["node", str(ROOT / "scripts/agent_host_probe.mjs"), str(config)], cwd=work,
                       env=env, check=True, timeout=90)


if __name__ == "__main__":
    main()
