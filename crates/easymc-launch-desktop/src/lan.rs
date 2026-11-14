//! LAN world discovery (Minecraft "Open to LAN" broadcast).
//!
//! Vanilla clients announce a hosted world as a UDP datagram sent to the
//! multicast group `224.0.2.60:4445`:
//!
//! ```text
//! [MOTD]Steve's world[/MOTD][AD]54321[/AD]
//! ```
//!
//! This module listens for those packets and returns de-duplicated worlds.
//! It does not need a running game client on this machine — any LAN peer works.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};

/// Multicast group used by Minecraft Java Edition.
pub const LAN_MULTICAST_IP: Ipv4Addr = Ipv4Addr::new(224, 0, 2, 60);
/// Port used by Minecraft Java Edition LAN announcements.
pub const LAN_MULTICAST_PORT: u16 = 4445;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanWorld {
    /// World name shown in the game.
    pub motd: String,
    /// IPv4 address the announcement came from.
    pub host: String,
    /// Game port to connect to.
    pub port: u16,
    /// Unix timestamp (seconds) when this announcement was last seen.
    pub last_seen: i64,
}

/// Parse one announcement packet. Returns `None` when there is no usable `[AD]port[/AD]`.
pub fn parse_announcement(packet: &[u8]) -> Option<(String, u16)> {
    let text = String::from_utf8_lossy(packet);
    let motd = between(&text, "[MOTD]", "[/MOTD]")
        .unwrap_or("")
        .trim()
        .to_string();
    let port: u16 = between(&text, "[AD]", "[/AD]")?.trim().parse().ok()?;
    if port == 0 {
        return None;
    }
    Some((motd, port))
}

fn between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let begin = s.find(start)? + start.len();
    let rest = &s[begin..];
    let finish = rest.find(end)?;
    Some(&rest[..finish])
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Listen for LAN announcements for `seconds` and return unique worlds.
///
/// Blocks the calling thread, so callers on an async runtime should use
/// `spawn_blocking`. Bind errors (port in use / no multicast route) are
/// returned so the UI can show a real failure instead of an empty list.
pub fn listen_blocking(seconds: u64) -> std::io::Result<Vec<LanWorld>> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, LAN_MULTICAST_PORT);
    socket.bind(&bind_addr.into())?;
    socket.join_multicast_v4(&LAN_MULTICAST_IP, &Ipv4Addr::UNSPECIFIED)?;
    socket.set_read_timeout(Some(Duration::from_millis(250)))?;
    let socket: UdpSocket = socket.into();

    let deadline = Instant::now() + Duration::from_secs(seconds.clamp(1, 30));
    let mut seen: HashMap<(String, u16), LanWorld> = HashMap::new();
    let mut buf = [0u8; 4096];

    while Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((n, addr)) => {
                if let Some((motd, port)) = parse_announcement(&buf[..n]) {
                    let host = addr.ip().to_string();
                    seen.insert(
                        (host.clone(), port),
                        LanWorld {
                            motd: if motd.is_empty() {
                                "Minecraft 局域网世界".to_string()
                            } else {
                                motd
                            },
                            host,
                            port,
                            last_seen: unix_now(),
                        },
                    );
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(e) => return Err(e),
        }
    }

    let mut worlds: Vec<LanWorld> = seen.into_values().collect();
    worlds.sort_by(|a, b| a.motd.to_lowercase().cmp(&b.motd.to_lowercase()));
    Ok(worlds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vanilla_announcement() {
        let (motd, port) = parse_announcement(b"[MOTD]Steve's world[/MOTD][AD]54321[/AD]").unwrap();
        assert_eq!(motd, "Steve's world");
        assert_eq!(port, 54321);
    }

    #[test]
    fn parses_without_motd() {
        let (motd, port) = parse_announcement(b"[AD]25565[/AD]").unwrap();
        assert_eq!(motd, "");
        assert_eq!(port, 25565);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_announcement(b"hello").is_none());
        assert!(parse_announcement(b"[AD]not-a-port[/AD]").is_none());
        assert!(parse_announcement(b"[AD]0[/AD]").is_none());
    }
}
