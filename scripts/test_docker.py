"""Exercise the real Docker image and Compose service; Python standard library only."""

import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import tempfile
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
IMAGE = "rekora:local"


def run(*args, env=None, input=None):
    try:
        return subprocess.run(args, env=env, input=input, text=True, capture_output=True,
                              check=True, timeout=120).stdout
    except subprocess.CalledProcessError as error:
        raise RuntimeError(f"Container command failed: {error.stdout}\n{error.stderr}") from error


def rpc(url, token, message, session=None):
    headers = {"Content-Type": "application/json",
               "Accept": "application/json, text/event-stream",
               "Authorization": f"Bearer {token}"}
    if session:
        headers.update({"Mcp-Session-Id": session, "MCP-Protocol-Version": "2025-11-25"})
    request = urllib.request.Request(url, json.dumps(message).encode(), headers)
    with urllib.request.urlopen(request, timeout=15) as response:
        data = response.read().decode()
        if "text/event-stream" in response.headers.get("Content-Type", ""):
            data = next(line[5:].strip() for line in data.splitlines()
                        if line.startswith("data:") and line[5:].strip())
        return json.loads(data) if data else None, response.headers.get("Mcp-Session-Id")


def main():
    with tempfile.TemporaryDirectory(prefix="rekora-docker-") as directory:
        # The default image user must be able to initialise the bind-mounted store.
        os.chmod(directory, 0o777)
        mount = f"{directory}:/workspace"
        user = f"{os.getuid()}:{os.getgid()}" if hasattr(os, "getuid") else "10001:10001"
        base = ["docker", "run", "--rm", "--user", user, "-v", mount, IMAGE]
        run(*base, "init")
        init = {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-11-25", "capabilities": {},
            "clientInfo": {"name": "docker-smoke", "version": "1"}}}
        output = run("docker", "run", "--rm", "-i", "--user", user, "-v", mount, IMAGE,
                     "mcp", "serve", "--stdio", input=json.dumps(init) + "\n" +
                     json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        assert any(json.loads(line).get("result", {}).get("serverInfo", {}).get("name") == "rekora"
                   for line in output.splitlines()), output
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        env = dict(os.environ, REKORA_PROJECT=directory, REKORA_PORT=str(port),
                   REKORA_USER=user,
                   REKORA_BIND_ADDRESS="127.0.0.1", REKORA_MCP_TOKEN=secrets.token_hex(32))
        compose = ["docker", "compose", "-f", str(ROOT / "compose.yaml"),
                   "-p", "rekora-smoke-" + secrets.token_hex(4)]
        url = f"http://127.0.0.1:{port}/mcp"
        try:
            run(*compose, "up", "-d", "--no-build", "--wait", env=env)
            try:
                rpc(url, "invalid", init)
                raise AssertionError("unauthenticated MCP request accepted")
            except urllib.error.HTTPError as error:
                assert error.code == 401
            result, session = rpc(url, env["REKORA_MCP_TOKEN"], init)
            assert result["result"]["serverInfo"]["name"] == "rekora"
            assert session
            rpc(url, env["REKORA_MCP_TOKEN"],
                {"jsonrpc": "2.0", "method": "notifications/initialized"}, session)
            result, _ = rpc(url, env["REKORA_MCP_TOKEN"], {
                "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
                    "name": "remember", "arguments": {"content": "Docker invoices archived for ten years"}}}, session)
            assert not result["result"].get("isError"), result
            # Recreate the container; only the bind-mounted database survives.
            run(*compose, "up", "-d", "--no-build", "--force-recreate", "--wait", env=env)
            result = json.loads(run(*base, "--format", "json", "memory", "search", "docker invoices archived"))
            assert result["hits"][0]["memory"]["content"] == "Docker invoices archived for ten years"
            assert json.loads(run(*base, "--format", "json", "doctor"))["ok"]
            print(f"PASS: Docker stdio, authenticated HTTP, custom port {port}, Compose health, recreation persistence, doctor")
        finally:
            run(*compose, "down", "--volumes", env=env)


if __name__ == "__main__":
    main()
