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

Railway is the quickest way to run one. The whole process is about ten minutes,
and a relay is light enough to sit comfortably inside a free tier — it forwards
connection setup, not conversations.

### Before you start

You need three things:

- A [Railway](https://railway.app) account, with your GitHub account connected.
- A **backup phrase** for the relay's identity.
- The **network id** the relay will serve.

Generate the first two from the [protocol
repository](https://github.com/DriftingNarwhal/distributed-intranet):

```bash
cargo run -p intranet-harness -- identity new
```

That prints a BIP-39 backup phrase. **Keep it secret** — anyone holding it can
impersonate this relay to your network. Store it the way you would an SSH private
key.

Your network id is the 64-character hex string identifying the network you are
running this relay for. If you are standing up a new network, it is printed when
you create it.

### Step 1 — Create the project

1. Go to [railway.app/new](https://railway.app/new).
2. Choose **Deploy from GitHub repo**.
3. Pick this repository. If Railway cannot see it, click **Configure GitHub App**
   and grant access to it.

Railway will start a build immediately. It will fail or restart until you finish
step 2 — that is expected, not a problem.

### Step 2 — Set the variables

Open your service → **Variables** tab → **New Variable**, and add:

| Key | Value |
|---|---|
| `RELAY_PHRASE` | The backup phrase from above, in quotes if it contains spaces |
| `RELAY_NETWORK` | Your 64-character network id |
| `PORT` | `8080` |

`RELAY_PHRASE` and `RELAY_NETWORK` have no defaults on purpose. A relay that
invented an identity at boot would come back with a different peer id after every
deploy, silently breaking every bootstrap address anyone had written down.

`PORT` is Railway's convention for the port it routes public HTTP to. Here that
is the health endpoint, not the libp2p port.

### Step 3 — Set up networking

Service → **Settings** → **Networking**. You need **both** of these, and it is
easy to add only the first:

**Public Networking** → **Generate Domain**. Railway gives you something like
`di-relay-production.up.railway.app` and routes HTTPS to `PORT`. This serves the
health and peer-id endpoints.

**TCP Proxy** → **Add TCP Proxy** → enter port `4001`. Railway gives you a host
and port like `monorail.proxy.rlwy.net:54321`. This is the actual libp2p path —
the one peers connect through.

Without the domain, the health check fails and Railway keeps restarting the
service. Without the TCP proxy, the service looks perfectly healthy and no peer
can reach it.

### Step 4 — Check it came up

Visit `https://your-domain.up.railway.app/health`. You want:

```json
{"status":"ready","listening":["/ip4/0.0.0.0/tcp/4001", …]}
```

`{"status":"starting"}` with a `503` means the process is up but not yet
listening. If it stays that way, check the deploy logs — the relay prints every
address it binds.

Then visit `/peer-id` and copy the value:

```json
{"peer_id":"12D3KooWKiD4GjUwYGbXKkHkcHV4i5Wzbq69giWXuDjmv1XAMZx6"}
```

### Step 5 — Give the address to your network

Combine the TCP proxy host and port from step 3 with the peer id from step 4:

```
/dns4/monorail.proxy.rlwy.net/tcp/54321/p2p/12D3KooWKiD4GjUwYGbXKkHkcHV4i5Wzbq69giWXuDjmv1XAMZx6
```

That is the bootstrap address members configure. Note the port is the **proxy's**
port, not 4001 — 4001 is what the container listens on inside Railway.

Read the peer id from the HTTPS endpoint rather than trusting whatever answers on
the TCP port. That is why the two are exposed separately: it lets a member
confirm they are reaching the relay they intended rather than something sitting
in its place.

### Step 6 — Consider running a second one

Nothing in steady-state operation depends on any particular relay, and a network
can designate several. Running two in different places costs very little and
removes the only piece of shared infrastructure a network has.

### Troubleshooting

**Build fails while fetching the protocol repository.** The build needs read
access to `distributed-intranet`. If it is private, add a GitHub token with
`repo` scope as a build variable, or make that repository public.

**Health check times out during deploy.** Almost always a missing public domain
(step 3) or a `PORT` that does not match the one Railway is routing to.

**Health is `ready` but peers cannot connect.** The TCP proxy is missing, or is
pointed at a port other than `4001`. Confirm with
`nc -vz monorail.proxy.rlwy.net 54321`.

**Peers connect but relayed connections fail.** See `RELAY_PUBLIC_ADDR` under
Configuration. This is the failure that looks like nothing is wrong: direct
connections keep working and health keeps reporting ready.

**The peer id changed after a deploy.** `RELAY_PHRASE` is missing or was edited.
Every bootstrap address referencing the old id is now stale.

## Configuration

| Variable | Required | Default | Purpose |
|---|---|---|---|
| `RELAY_PHRASE` | yes¹ | — | BIP-39 backup phrase for the relay's identity |
| `RELAY_SEED` | no¹ | — | Seed byte 0–255, for reproducible local runs only |
| `RELAY_NETWORK` | yes | — | Network id: 64 hex characters, or 0–255 for local runs |
| `PORT` | no | `8080` | Health and peer-id endpoint |
| `RELAY_PORT` | no | `4001` | libp2p port, TCP and QUIC |
| `RELAY_LISTEN` | no | dual-stack on `RELAY_PORT` | Comma-separated multiaddresses, overriding the default entirely |
| `RELAY_PUBLIC_ADDR` | no² | — | Comma-separated addresses to announce. `host:port` is accepted and converted |
| `RELAY_MAX_RESERVATIONS` | no | `128` | Concurrent reservations across all peers |
| `RELAY_MAX_RESERVATIONS_PER_IDENTITY` | no | `4` | Concurrent reservations per identity |
| `RELAY_MAX_CIRCUITS` | no | `32` | Concurrent relayed circuits |

² Not required by the process, and required in practice behind any proxy or load
balancer — see below.

¹ Set `RELAY_PHRASE` for anything real. `RELAY_SEED` derives a fully predictable
identity from a single byte and exists only so a local test can be reproducible.

### About `RELAY_PUBLIC_ADDR`

**On Railway you need it.** Paste the **TCP Proxy** value from Settings → Networking
exactly as shown — `monorail.proxy.rlwy.net:54321`. Both forms work:

```
RELAY_PUBLIC_ADDR=monorail.proxy.rlwy.net:54321
RELAY_PUBLIC_ADDR=/dns4/monorail.proxy.rlwy.net/tcp/54321
```

Do **not** use the generated **domain** (`*.up.railway.app`). That is HTTPS for the
health endpoint and cannot carry libp2p. Do not use an address from the deployment
logs either: those are the container's own, and they change on every deploy.

Confirm it took by opening `/health` — `announcing` should list what you set. If it
is empty, the variable is not reaching the process, and startup logs a warning
saying so.

Usually, elsewhere, you do not need it. The relay announces its own routable listen
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

## The protocol this serves

This repository is only the relay. The protocol it belongs to — what a network
is, how membership and governance work, how content is stored, encrypted,
searched and served — lives in
**[DriftingNarwhal/distributed-intranet](https://github.com/DriftingNarwhal/distributed-intranet)**.

Start with
[its README](https://github.com/DriftingNarwhal/distributed-intranet#readme),
which covers what the project is, what you can build on it, and what it
deliberately is not. The design itself is six specification documents in
[`specs/`](https://github.com/DriftingNarwhal/distributed-intranet/tree/main/specs);
relays are Core Protocol Spec §5.2–5.5.

You do not need to read any of it to run a relay. You will want it if you are
deciding whether to run one, or wondering why a relay is allowed to be untrusted.

### Why this is a wrapper and not a relay implementation

The relay itself is `intranet_transport::RelayNode`, pulled from that repository
at tag `v1.0.0`. This binary only reads configuration, serves health, starts it,
and shuts down cleanly.

That is deliberate. `RelayNode` is covered by the protocol repository's
conformance suite — its reservation and circuit ceilings are asserted against a
*live* relay rather than against a model of one, because a limiter that computes
a decision and never enforces it is a real defect that has been found in relay
code before, including in an earlier relay this one was modelled on. A
hand-rolled relay here would be a second copy of that logic, free to drift from
the copy the tests actually exercise.

## Licence

**[GNU Affero General Public License v3.0](LICENSE)** — (c) DriftingNarwhal.

Free to run, modify and share. The condition is that it stays that way: **if you run a
modified version of this relay for other people, you must publish the complete
corresponding source under the same licence.** That is the Affero clause, and it is the
whole reason this repository is AGPL rather than something weaker — a relay is a service by
definition, so a licence that only triggered on *distribution* would never trigger at all
here. Anyone whose traffic depends on a relay should be able to obtain the source of the
relay they are depending on.

Note this does not reach the network it serves: a relay carries bytes for members and holds
no state, so running one places no licence obligation on anybody's client or content.

The protocol crates this links are
[MPL-2.0](https://github.com/DriftingNarwhal/distributed-intranet), which MPL §3.3 permits
being combined into an AGPL work.
