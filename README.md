# DI-Relay

A bootstrap relay for a [Distributed Intranet](https://github.com/DriftingNarwhal/distributed-intranet)
network. Deployable to Railway in a few minutes, and cheap enough to sit inside a
free tier.

## What a relay is for, and what it is not

When two members are both behind NAT, neither can dial the other. A relay solves
that cold start: both connect *out* to it, it introduces them, and they then
attempt a direct connection through it — first a hole punch, and only if that
fails does traffic keep flowing over the circuit.

**A relay establishes connections; it does not carry traffic.** Circuits are
capped at 120 seconds and 8 MB by default, and hitting those ceilings is expected
rather than exceptional. That is why running one is cheap, and why it is not a
bandwidth commitment. If you want the ceilings, see `RELAY_MAX_*` below.

Three further things follow from that, and they are the reason this is worth
running:

- **It holds no state.** Nothing survives a restart. Its identity comes from
  configuration and its reservations are re-established by clients.
- **It is not trusted.** It never holds a network's keys and cannot read
  anything passing through it. A relay is infrastructure, not a member.
- **It is disposable.** A network can designate several, and losing one costs a
  reconnection rather than a network. Nothing in steady-state operation depends
  on any particular relay staying up.

## Deploy to Railway

**Before you start**, you need two values: a **backup phrase** for the relay's
identity, and the **network id** it serves. Generate both with the harness from
the protocol repository:

```bash
cargo run -p intranet-harness -- identity new
```

Keep the phrase secret — anyone holding it can impersonate this relay to your
network.

### 1. Create the service

Railway → **New Project** → **Deploy from GitHub repo** → select this repository.

### 2. Set variables before the first deploy

Service → **Variables**:

| Key | Value |
|---|---|
| `RELAY_PHRASE` | The backup phrase from above |
| `RELAY_NETWORK` | Your network id (64 hex characters) |
| `PORT` | `8080` |

The service will fail to start without the first two, deliberately — a relay
that generated a fresh identity on boot would get a new peer id every deploy and
silently invalidate every bootstrap address anyone had recorded.

### 3. Set up networking

Settings → **Networking**. You need **both**:

- **Public Networking → Generate Domain.** Gives you an HTTPS domain routed to
  `PORT`. This serves `/health` and `/peer-id`, which is how a client confirms it
  is reaching the relay it intends to.
- **TCP Proxy → Add TCP Proxy**, port `4001`. Gives you a host and port like
  `monorail.proxy.rlwy.net:54321`. This is the actual libp2p path.

Without the domain the health check fails; without the TCP proxy nothing can
connect. They are separate entries and it is easy to set only the first.

### 4. Tell your network about it

Take the peer id from `https://your-domain/peer-id` and the TCP proxy host and
port from step 3. The bootstrap address is:

```
/dns4/monorail.proxy.rlwy.net/tcp/54321/p2p/<peer-id>
```

Verify the peer id over the HTTPS endpoint rather than trusting whatever answers
on the TCP port — that check is the point of exposing it separately.

## Configuration

| Variable | Required | Default | Purpose |
|---|---|---|---|
| `RELAY_PHRASE` | yes¹ | — | BIP-39 backup phrase for the relay's identity |
| `RELAY_SEED` | no¹ | — | Seed byte 0–255, for reproducible local runs only |
| `RELAY_NETWORK` | yes | — | Network id: 64 hex characters, or 0–255 for local runs |
| `PORT` | no | `8080` | Health and peer-id endpoint |
| `RELAY_PORT` | no | `4001` | libp2p port, TCP and QUIC |
| `RELAY_LISTEN` | no | dual-stack on `RELAY_PORT` | Comma-separated multiaddresses, overriding the default entirely |
| `RELAY_PUBLIC_ADDR` | no | — | Comma-separated multiaddresses to announce |
| `RELAY_MAX_RESERVATIONS` | no | `128` | Concurrent reservations across all peers |
| `RELAY_MAX_RESERVATIONS_PER_IDENTITY` | no | `4` | Concurrent reservations per identity |
| `RELAY_MAX_CIRCUITS` | no | `32` | Concurrent relayed circuits |

¹ Set `RELAY_PHRASE` for anything real. `RELAY_SEED` derives a fully predictable
identity from a single byte and exists only so a local test can be reproducible.

### About `RELAY_PUBLIC_ADDR`

Usually you do not need it. The relay announces its own routable listen
addresses, and a client builds its circuit address from whatever address it used
to reach the relay — so behind Railway's TCP proxy the right thing already
happens.

Set it when the relay has **no** routable listen address of its own, which is the
case behind some load balancers and in some container networks. A relay with no
external address at all still accepts reservations, but clients reject them for
naming no addresses, so tiers 2 and 3 fail while direct connections keep working
and the health check keeps reporting ready. If relayed connections are failing
and nothing looks wrong, this is the first thing to check.

## Endpoints

| Path | Response |
|---|---|
| `/health` | `200` with `{"status":"ready","listening":[…]}` once listening; `503` with `{"status":"starting"}` before |
| `/peer-id` | `{"peer_id":"12D3Koo…"}`, or `null` before the identity loads |
| `/` | A plain string |

`/health` returns **503 until the relay is actually listening**, not merely
until the process is up. A relay whose listeners failed to bind is running and
answering HTTP while being completely unreachable; reporting `200` there would
mean the platform never restarts it, which is the kind of failure that hides
longest.

## Running locally

```bash
RELAY_SEED=1 RELAY_NETWORK=42 PORT=8080 cargo run --release
```

Then `curl localhost:8080/health`. It answers `503` until the listeners are up
and `200` after, so this is also the quickest way to see whether the port is
already taken.

## How this relates to the protocol repository

The relay itself is `intranet_transport::RelayNode`, not a reimplementation. This
repository is a deployment wrapper: configuration, a health endpoint, and clean
shutdown.

That is deliberate. `RelayNode` is covered by the upstream conformance suite —
its reservation and circuit ceilings are asserted against a live relay rather
than against a model of one, because a limiter that computes a decision and never
enforces it is a real defect that has been found in relay code before. A
hand-rolled relay here would be a second copy of that logic, free to drift from
the copy the tests actually exercise.

## Licence

MIT OR Apache-2.0, matching the protocol repository.
