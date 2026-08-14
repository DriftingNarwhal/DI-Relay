//! Bootstrap relay for a Distributed Intranet network — Core Protocol Spec
//! §5.2–5.5.
//!
//! # What this binary is, and deliberately is not
//!
//! It is a deployment wrapper. The relay itself is
//! [`intranet_transport::RelayNode`], which is covered by the upstream
//! conformance suite: it enforces §5.3's reservation and circuit ceilings
//! against a live relay, and announces external addresses so reservations are
//! usable. Reimplementing any of that here would produce a second copy free to
//! drift from the one the tests exercise, which is how a relay ends up
//! enforcing limits it only appears to have.
//!
//! So this file does four things and nothing else: read configuration, serve a
//! health endpoint, start the relay, and shut down cleanly.
//!
//! # A relay is disposable on purpose
//!
//! §5.4: no state survives a restart. Its keypair comes from configuration, its
//! reservations are re-established by clients, and losing it costs a
//! reconnection rather than a network. That is what makes it cheap to run and
//! interchangeable with any other relay a network designates — and it is why
//! there is nothing to flush on shutdown.

use intranet_identity::{MasterSeed, NetworkId, PerNetworkIdentity};
use intranet_transport::{NodeEvent, RelayLimits, RelayNode};
use intranet_transport::Multiaddr;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// What the health endpoint reports.
#[derive(Clone, Default)]
struct Health {
    /// True only once the relay is actually listening somewhere.
    ///
    /// Not "the process started". A relay whose listeners all failed to bind is
    /// running, answering HTTP, and useless — and if health reported ready it
    /// would never be restarted, which is the failure mode that hides longest.
    ready: bool,
    peer_id: Option<String>,
    listening: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let health = Arc::new(Mutex::new(Health::default()));

    // Served before anything else can fail. A hosting platform's health check
    // starts immediately, and a slow boot that answers nothing is
    // indistinguishable from a crashed deployment.
    let http_port = env_u16("PORT").unwrap_or(8080);
    spawn_health_endpoint(http_port, Arc::clone(&health));
    println!("health-port: {http_port}");

    let identity = load_identity()?;
    let mut relay = RelayNode::with_limits(&identity, limits_from_env()?)
        .map_err(|e| format!("could not build relay: {e}"))?;
    let peer_id = relay.peer_id().to_string();
    health.lock().expect("health lock").peer_id = Some(peer_id.clone());

    // §5.4: the peer id has to be verifiable out of band, so whoever adds this
    // relay as a bootstrap candidate can confirm they reached the relay they
    // meant to rather than something answering in its place.
    println!("peer-id: {peer_id}");

    // Announced before listening, so the first reservation already has an
    // address list to return.
    for address in multiaddrs_from_env("RELAY_PUBLIC_ADDR")? {
        println!("announcing: {address}");
        relay.add_public_address(address);
    }

    let listen = match multiaddrs_from_env("RELAY_LISTEN")? {
        addresses if addresses.is_empty() => default_listen(env_u16("RELAY_PORT").unwrap_or(4001))?,
        addresses => addresses,
    };
    for address in listen {
        relay
            .listen_on(address.clone())
            .map_err(|e| format!("could not listen on {address}: {e}"))?;
    }

    run(relay, peer_id, health).await
}

async fn run(
    mut relay: RelayNode,
    peer_id: String,
    health: Arc<Mutex<Health>>,
) -> Result<(), String> {
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            biased;
            () = &mut shutdown => {
                println!("shutting down");
                // Nothing to flush: a relay holds no state across restarts.
                return Ok(());
            }
            event = relay.next_event() => match event {
                NodeEvent::Listening(address) => {
                    println!("listening: {address}/p2p/{peer_id}");
                    let mut guard = health.lock().expect("health lock");
                    guard.listening.push(address.to_string());
                    // Ready means reachable, and this is the first moment that
                    // is true rather than merely intended.
                    guard.ready = true;
                }
                NodeEvent::Connected { peer, tier, .. } => {
                    println!("connected: peer={peer} tier={}", tier.label());
                }
                NodeEvent::Disconnected { peer } => {
                    println!("disconnected: peer={peer}");
                }
                // Refusals are logged as loudly as grants. An unenforced limiter
                // and an enforced one look identical from outside unless the
                // refusals are visible, and that is a defect this project has
                // already found in a relay once.
                NodeEvent::ReservationGranted { peer } => {
                    println!("reservation-granted: peer={peer}");
                }
                NodeEvent::ReservationDenied { peer } => {
                    println!("reservation-denied: peer={peer}");
                }
                NodeEvent::ReservationReleased { peer } => {
                    println!("reservation-released: peer={peer}");
                }
                NodeEvent::DialFailed { peer, error } => match peer {
                    Some(peer) => println!("dial-failed: peer={peer} error={error}"),
                    None => println!("dial-failed: error={error}"),
                },
                _ => {}
            },
        }
    }
}

/// Resolves the relay's identity from configuration.
///
/// A backup phrase is the intended production input; the seed byte exists so a
/// local run is reproducible without minting a phrase. Neither is optional:
/// generating a fresh identity on boot would give the relay a new peer id every
/// deploy, silently invalidating every bootstrap address anyone had recorded.
fn load_identity() -> Result<PerNetworkIdentity, String> {
    let network = match std::env::var("RELAY_NETWORK") {
        Ok(value) => parse_network(&value)?,
        Err(_) => return Err("RELAY_NETWORK is required".into()),
    };

    let master = match (std::env::var("RELAY_PHRASE"), std::env::var("RELAY_SEED")) {
        (Ok(phrase), _) => MasterSeed::from_backup_phrase(phrase.trim())
            .map_err(|e| format!("RELAY_PHRASE is not a valid backup phrase: {e}"))?,
        (Err(_), Ok(seed)) => {
            let byte: u8 = seed
                .trim()
                .parse()
                .map_err(|_| "RELAY_SEED must be a number from 0 to 255".to_string())?;
            MasterSeed::from_entropy([byte; 32])
        }
        (Err(_), Err(_)) => {
            return Err("set RELAY_PHRASE (production) or RELAY_SEED (local testing)".into());
        }
    };

    master
        .identity_for(&network)
        .map_err(|e| format!("could not derive the relay identity: {e}"))
}

/// Parses a network identifier: 64 hex characters, or a small integer.
///
/// The integer form is a convenience for local runs and matches the harness, so
/// a relay and a test client can be pointed at the same network without anyone
/// copying a 64-character string by hand.
fn parse_network(value: &str) -> Result<NetworkId, String> {
    let value = value.trim();
    if let Ok(small) = value.parse::<u8>() {
        return Ok(NetworkId::from_bytes([small; 32]));
    }
    if value.len() != 64 {
        return Err("RELAY_NETWORK must be 64 hex characters or a number from 0 to 255".into());
    }
    let mut bytes = [0u8; 32];
    for (index, pair) in value.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(pair).map_err(|_| "RELAY_NETWORK is not valid hex")?;
        bytes[index] =
            u8::from_str_radix(text, 16).map_err(|_| "RELAY_NETWORK is not valid hex")?;
    }
    Ok(NetworkId::from_bytes(bytes))
}

/// Dual-stack TCP and QUIC on a **fixed** port.
///
/// Deliberately not `RelayNode::listen_default`, which binds ephemeral ports.
/// That is right for a test or a member node, and wrong for a deployed relay:
/// the whole point of a relay is that its address is written down somewhere and
/// dialled later, and a hosting platform's TCP proxy is configured to forward to
/// one specific port. An ephemeral port would leave the relay running, healthy,
/// and unreachable at the address everyone was told to use.
fn default_listen(port: u16) -> Result<Vec<Multiaddr>, String> {
    [
        format!("/ip6/::/tcp/{port}"),
        format!("/ip4/0.0.0.0/tcp/{port}"),
        format!("/ip6/::/udp/{port}/quic-v1"),
        format!("/ip4/0.0.0.0/udp/{port}/quic-v1"),
    ]
    .iter()
    .map(|address| {
        address
            .parse()
            .map_err(|e| format!("could not build listen address {address}: {e}"))
    })
    .collect()
}

/// Reads the §5.3 resource ceilings, defaulting to the specified values.
///
/// Overridable because §5.3 calls them baseline defaults rather than constants,
/// and a relay operator knows their own capacity. Raising them is a deliberate
/// act, which is why each has its own variable rather than a single "limits"
/// blob nobody reads before editing.
fn limits_from_env() -> Result<RelayLimits, String> {
    let defaults = RelayLimits::default();
    Ok(RelayLimits {
        max_reservations: env_u32("RELAY_MAX_RESERVATIONS")?.unwrap_or(defaults.max_reservations),
        max_reservations_per_identity: env_u32("RELAY_MAX_RESERVATIONS_PER_IDENTITY")?
            .unwrap_or(defaults.max_reservations_per_identity),
        max_circuits: env_u32("RELAY_MAX_CIRCUITS")?.unwrap_or(defaults.max_circuits),
        ..defaults
    })
}

fn env_u16(name: &str) -> Option<u16> {
    std::env::var(name).ok()?.trim().parse().ok()
}

fn env_u32(name: &str) -> Result<Option<u32>, String> {
    match std::env::var(name) {
        Err(_) => Ok(None),
        Ok(value) => value
            .trim()
            .parse()
            .map(Some)
            .map_err(|_| format!("{name} must be a whole number")),
    }
}

/// Reads a comma-separated list of multiaddresses.
fn multiaddrs_from_env(name: &str) -> Result<Vec<Multiaddr>, String> {
    let Ok(value) = std::env::var(name) else {
        return Ok(Vec::new());
    };
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            entry
                .parse()
                .map_err(|e| format!("{name}: '{entry}' is not a valid multiaddress: {e}"))
        })
        .collect()
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let Ok(mut sigterm) = signal(SignalKind::terminate()) else {
            let _ = tokio::signal::ctrl_c().await;
            return;
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// A deliberately tiny HTTP responder.
///
/// Hand-rolled rather than pulling in a web framework: it answers three fixed
/// questions, and every dependency here is build time a relay operator pays on
/// every deploy.
fn spawn_health_endpoint(port: u16, health: Arc<Mutex<Health>>) {
    tokio::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("fatal: could not bind health port {port}: {error}");
                // Exiting rather than continuing: a relay whose health endpoint
                // never binds will be marked unhealthy and restarted anyway, and
                // failing immediately makes the reason visible in the logs
                // instead of leaving a silent timeout to be interpreted.
                std::process::exit(1);
            }
        };
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                continue;
            };
            let snapshot = health.lock().expect("health lock").clone();

            let mut buffer = [0u8; 1024];
            let read = socket.read(&mut buffer).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]);
            let path = request.split_whitespace().nth(1).unwrap_or("/");

            let (status, body) = if path.starts_with("/peer-id") {
                let peer_id = snapshot
                    .peer_id
                    .map(|id| format!("\"{id}\""))
                    .unwrap_or_else(|| "null".into());
                ("200 OK", format!("{{\"peer_id\":{peer_id}}}"))
            } else if path.starts_with("/health") {
                let listening = snapshot
                    .listening
                    .iter()
                    .map(|address| format!("\"{address}\""))
                    .collect::<Vec<_>>()
                    .join(",");
                let body = format!(
                    "{{\"status\":\"{}\",\"listening\":[{listening}]}}",
                    if snapshot.ready { "ready" } else { "starting" }
                );
                // 503 until the relay is actually listening. A platform health
                // check exists to distinguish working from not, and answering
                // 200 while unreachable defeats it — the relay would never be
                // restarted, because nothing would know it was broken.
                (
                    if snapshot.ready {
                        "200 OK"
                    } else {
                        "503 Service Unavailable"
                    },
                    body,
                )
            } else {
                ("200 OK", "Distributed Intranet relay".to_string())
            };

            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
}
