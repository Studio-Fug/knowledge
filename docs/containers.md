# Containerized cache

One non-root Rust cache process, a persistent `/data` volume, and an HTTP health
check. The image contains no host-specific paths, network identity, or credentials.
Docker with Compose v2 is required. Linux, macOS, and Windows hosts can use Docker's
Linux containers. Image builds are tested natively on Linux amd64 and arm64.

## Launch on a tailnet

The Rust launcher accepts the authentication key at launch time, either through
`TS_AUTHKEY` or a file. From the repository:

```sh
cargo run --locked --bin knowledge-launch -- --authkey-file /path/to/authkey up
```

Alternatively, without installing Rust on the host:

```sh
TS_AUTHKEY='your-auth-key' docker compose -p knowledge -f compose.yaml -f deploy/compose.tailnet.yaml up -d --build --wait --wait-timeout 120
```

Use a non-ephemeral auth key appropriate to your own tailnet policy. Device
approval and Serve policy, if required by your account, are configured there.
The launcher does not change ACLs, advertise subnet routes, or enable Funnel.
Use `--hostname NAME` to select a node name; the default is `knowledge`.
With Compose directly, set `KNOWLEDGE_HOSTNAME` instead.

Once connected, clients allowed by your tailnet policy can use:

```sh
curl http://knowledge:8787/health
curl -H 'Content-Type: application/json' -d '{}' http://knowledge:8787/v1/search
```

Use the node's assigned IP or full DNS name if short-name resolution is unavailable.
These are HTTP requests carried over the encrypted tailnet connection.

The separate Tailscale container runs in userspace mode. Serve forwards tailnet
TCP port 8787 to Knowledge on `127.0.0.1:8787` in their shared network namespace.
There are no published host ports, no host TUN device, and no added networking
capabilities. This integration provides inbound access; userspace mode does not
give the cache ordinary outbound routes to other tailnet nodes.

Authentication keys are never baked into images or written to repository files by
the launcher. The sidecar receives its key through its runtime environment;
Docker administrators can inspect that environment. Protect the key file and
the sidecar's state volume. The image uses a versioned Tailscale sidecar by default;
`TAILSCALE_IMAGE` can select a different version or digest.

## Enable publication

Reads are available to clients that can reach the cache. Writes remain disabled
until you configure allowed Ed25519 publisher keys:

```sh
cargo run --locked --bin knowledge-launch -- --authkey-file /path/to/authkey --allow-publisher PUBLIC_KEY up
```

Repeat `--allow-publisher` for additional keys. For Compose directly, supply
`KNOWLEDGE_PUBLISHERS` as a comma-separated list of public keys. Clients still sign
their own artifacts before `POST /v1/artifacts`; the routing sidecar does not sign
content or bypass publication checks. Being an allowed publisher does not establish
evidence truth or requester trust. This remains a public-artifact cache with no
confidential-content access or payment implementation.

## Local launch

```sh
cargo run --locked --bin knowledge-launch -- --mode local up
```

Or:

```sh
docker compose -p knowledge -f compose.yaml -f deploy/compose.local.yaml up -d --build --wait
```

The local overlay publishes only `127.0.0.1:8787`. `KNOWLEDGE_PORT` changes the
host port without changing the container port. The standalone cache image can
also be used behind any other network or reverse-proxy arrangement:

```sh
docker build -t knowledge:local .
docker run -d --init --name knowledge --restart unless-stopped --read-only --cap-drop ALL --security-opt no-new-privileges:true -p 127.0.0.1:8787:8787 -v knowledge-data:/data knowledge:local
```

The image defaults to serving on all **container** interfaces on port 8787.
`KNOWLEDGE_CACHE`, `KNOWLEDGE_LISTEN`, `KNOWLEDGE_ALLOW_NETWORK`, and
`KNOWLEDGE_PUBLISHERS` configure the process; explicit CLI arguments override them.
Override the image command to run other Knowledge CLI operations.

## Persistence and management

```sh
cargo run --locked --bin knowledge-launch -- status
cargo run --locked --bin knowledge-launch -- logs
cargo run --locked --bin knowledge-launch -- down
```

Use the same `--mode`, `--project`, and `--directory` options on each invocation.
`down` preserves named volumes. In tailnet mode these hold cache data and node
identity separately. Subsequent launches can omit the auth key while the node
remains authenticated; `TS_AUTH_ONCE=true` avoids reusing a one-time key at restart.
Deleting volumes destroys the corresponding data or identity. Back up cache data
with the cache stopped, and keep node-state backups confidential.

The cache runs as UID/GID 10001. Fresh named volumes inherit `/data` ownership;
bind-mounted directories must be writable by that identity. Its root filesystem
can be read-only, and it needs no added capabilities. One container process owns
each cache volume; do not share it between running replicas.

## Images and validation

Main builds publish `ghcr.io/studio-fug/knowledge:latest` and a commit-addressed
tag after Rust and native container tests pass. Both support amd64 and arm64.
Registry access follows the package's visibility; authenticate to GHCR if the
package is private. Building locally is always supported. To use the published
image with Compose, set `KNOWLEDGE_IMAGE` and omit building (`--no-build`).

CI tests real containers for non-root execution, read-only operation, HTTP access,
publisher restrictions, health checks, and persistence across container recreation.
It validates the composed sidecar wiring and forwarding configuration without
joining a tailnet. A live tailnet connectivity check needs an operator-provided
key and a second allowed tailnet client.
