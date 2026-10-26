# Docker and HTTP MCP

Each container serves one mounted project. SQLite lives in that project's `.rekora/`
directory and survives container replacement. Mount the project so source evidence
and Git metadata are available as well as the database. Initialise it explicitly;
starting the MCP server never creates a store on its own.

## Build and initialise

```sh
docker build -t rekora:local .
docker run --rm -v "/absolute/path/to/project:/workspace" rekora:local init
```

The image runs as UID/GID `10001:10001`. On Linux, initialise and run with
`--user "$(id -u):$(id -g)"` when the mounted directory belongs to your account.
For Compose set `REKORA_USER="$(id -u):$(id -g)"` in your environment.
The project directory must be writable for SQLite WAL, migrations and backups.
Docker Desktop handles Windows/macOS bind-mount permissions separately.

## Stdio: launch from an MCP client

No network port or authentication token is needed. Keep `-i`; omit `-t` because a
terminal corrupts the JSON-RPC stream. Configure the client with:

```json
{
  "mcpServers": {
    "rekora": {
      "command": "docker",
      "args": ["run", "--rm", "-i", "-v", "/absolute/path/to/project:/workspace", "rekora:local", "mcp", "serve", "--stdio"]
    }
  }
}
```

On Windows, use a Docker-compatible absolute path such as `C:/Projects/my-project`.
Use `docker run --rm -v ... rekora:local memory ...` for other CLI commands.
Host adapters continue to install the native stdio command; add the Docker entry
manually to your chosen client's project configuration.

## HTTP with a custom host port

Set a random secret with at least 32 printable ASCII characters. For example,
`python -c "import secrets; print(secrets.token_hex(32))"` generates one. In Bash:

```sh
export REKORA_MCP_TOKEN='paste-your-generated-token-here'
export REKORA_PROJECT='/absolute/path/to/project'
export REKORA_PORT=9876
docker compose up -d --build --wait
```

PowerShell:

```powershell
$env:REKORA_MCP_TOKEN = 'paste-your-generated-token-here'
$env:REKORA_PROJECT = 'C:/Projects/my-project'
$env:REKORA_PORT = '9876'
docker compose up -d --build --wait
```

Connect your Streamable HTTP MCP client to `http://127.0.0.1:9876/mcp`, supplying
`Authorization: Bearer <your-token>` on every request. Clients must handle MCP
sessions and JSON/SSE responses; this is not a raw TCP or legacy SSE-only server.
Never commit the token to client configuration. Use the client's secret/environment
integration for headers. `/healthz` is an unauthenticated liveness endpoint; it does
not check project initialisation. Use `rekora doctor` to check the store.

Compose variables:

| Variable | Default | Purpose |
|---|---|---|
| `REKORA_MCP_TOKEN` | required | Bearer credential for all MCP requests |
| `REKORA_PROJECT` | current repository | Project bind mount |
| `REKORA_PORT` | `8080` | Host port; container port remains `8080` |
| `REKORA_BIND_ADDRESS` | `127.0.0.1` | Interface on which Docker publishes the host port |
| `REKORA_USER` | `10001:10001` | Runtime UID:GID; set to your account on Linux bind mounts |

Native equivalent:

```sh
rekora --project /path/to/project mcp serve --http --listen 127.0.0.1:9876
```

For Docker without Compose, pass `-e REKORA_MCP_TOKEN`, the project mount, and
`-p 127.0.0.1:9876:8080`, then run `mcp serve --http --listen 0.0.0.0:8080`.
`--listen` accepts IPv4 or bracketed IPv6 socket addresses. Stdio remains the default;
`--stdio` and `--http` are mutually exclusive.

## Remote access and operation

Publishing beyond loopback is explicit. Put a TLS reverse proxy in front of the
service, preserve Authorization/session/protocol headers and SSE streaming, and
add the public hostname with `--allow-host memories.example.com` (repeatable).
Default permitted Host names are `localhost`, `127.0.0.1` and `::1`; ports are
accepted. Browser Origin headers are rejected; native MCP clients omit Origin.
There is no CORS, OAuth, per-user access control or multi-project HTTP routing.
The shared token grants the entire MCP tool surface for the mounted project.

HTTP bodies are limited to 1 MiB, in addition to existing per-tool limits. Compose
uses a read-only container filesystem, writable project mount and temporary `/tmp`,
dropped capabilities, no privilege escalation and a healthcheck. Do not mount the
Docker socket. Mount an entire compatible worktree layout if using linked worktrees;
a host `.git` file pointing outside the mount cannot be resolved inside a container.

```sh
docker compose logs --tail 100
docker compose down
```

Stopping/recreating the service keeps `.rekora/` on the host. Tokens are visible to
users with Docker inspection privileges. For backup/export, use Rekora's CLI rather
than copying an active SQLite file without its WAL. SIGINT/SIGTERM stop HTTP gracefully.

## Verify locally

```sh
docker build -t rekora:local .
python scripts/test_docker.py
```

The smoke test creates a temporary project and verifies stdio, authenticated HTTP,
a dynamically selected custom port, Compose health, container recreation persistence
and database integrity. It removes its containers and temporary project on exit.
