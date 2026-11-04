//! Published ports for Docker sessions (docs/SPEC.md → Ports): the preset checkboxes and the
//! normalizer every port list passes before it is saved, recorded or reaches `docker run -p`.
//!
//! Grammar (Docker's): `PORT` | `HOST:CONTAINER` | `IP:HOST:CONTAINER`, each port `1–65535` or
//! a range `A-B` (`A <= B`, at most 100 ports, host and container of equal length), optional `/tcp` or `/udp`,
//! IPv4 only. The canonical form always names the IP (`127.0.0.1` unless one was typed), drops
//! the default `/tcp` and writes a one-port range as the port: `3000` → `127.0.0.1:3000:3000`,
//! `0.0.0.0:8080:80/udp` stays. It is rebuilt from the parsed numbers, so no typed character
//! other than digits, `.`, `:`, `-` and `/udp` reaches the argv.
use crate::models::PortPreset;
use std::net::Ipv4Addr;

/// At most this many entries per list.
pub const MAX_PORTS: usize = 20;
/// At most this many ports per range entry (`A-B` with `B - A + 1`).
pub const MAX_RANGE_PORTS: u16 = 100;
/// Longer input is never a valid spec (`255.255.255.255:65535-65535:65535-65535/udp` is 43).
const MAX_SPEC_CHARS: usize = 64;
/// How much of a refused spec the error repeats.
const SHOWN_CHARS: usize = 32;
pub const TOO_MANY: &str = "Höchstens 20 Port-Angaben erlaubt.";
pub const DOCKER_ONLY: &str = "Ports lassen sich nur für Docker-Sessions freigeben.";

/// The dev-server ports the composer offers, in display order.
pub const PRESETS: [(u16, &str); 8] = [
    (3000, "Next.js · Node"),
    (5173, "Vite"),
    (4200, "Angular"),
    (8000, "Django · FastAPI"),
    (8080, "Spring · HTTP"),
    (5000, "Flask · ASP.NET"),
    (4321, "Astro"),
    (6006, "Storybook"),
];

pub fn presets() -> Vec<PortPreset> {
    PRESETS
        .iter()
        .map(|(port, label)| PortPreset {
            port: *port,
            label: (*label).to_string(),
        })
        .collect()
}

/// One parsed `-p` value; ranges are inclusive, a single port is `(n, n)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Spec {
    ip: Ipv4Addr,
    host: (u16, u16),
    container: (u16, u16),
    udp: bool,
}

impl Spec {
    fn canonical(&self) -> String {
        let range = |(first, last): (u16, u16)| {
            if first == last {
                first.to_string()
            } else {
                format!("{first}-{last}")
            }
        };
        let protocol = if self.udp { "/udp" } else { "" };
        format!(
            "{}:{}:{}{protocol}",
            self.ip,
            range(self.host),
            range(self.container)
        )
    }

    /// Docker binds a host port once per protocol whatever the IP (`0.0.0.0` covers all).
    fn overlap(&self, other: &Spec) -> Option<u16> {
        let first = self.host.0.max(other.host.0);
        let last = self.host.1.min(other.host.1);
        (self.udp == other.udp && first <= last).then_some(first)
    }
}

/// The trimmed input for an error message: printable ASCII only (no control or format
/// characters such as U+202E or U+200B), at most 32 characters.
fn shown(spec: &str) -> String {
    let clean: String = spec.chars().filter(|c| matches!(c, ' '..='~')).collect();
    if clean.chars().count() > SHOWN_CHARS {
        let cut: String = clean.chars().take(SHOWN_CHARS).collect();
        format!("{cut}…")
    } else {
        clean
    }
}

fn invalid(spec: &str) -> String {
    format!("Port „{}“ ist ungültig.", shown(spec))
}

enum PortError {
    Invalid,
    OutOfRange,
}

fn port(text: &str) -> Result<u16, PortError> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(PortError::Invalid);
    }
    // More than five digits is out of range whatever the value (leading zeros included).
    let value = (text.len() <= 5)
        .then(|| text.parse::<u16>().ok())
        .flatten();
    value.filter(|port| *port > 0).ok_or(PortError::OutOfRange)
}

fn range(text: &str) -> Result<(u16, u16), PortError> {
    let (first, last) = match text.split_once('-') {
        Some((first, last)) => (port(first)?, port(last)?),
        None => {
            let single = port(text)?;
            (single, single)
        }
    };
    if first > last {
        return Err(PortError::Invalid);
    }
    Ok((first, last))
}

fn parse(spec: &str) -> Result<Spec, String> {
    let fail = |error: PortError| match error {
        PortError::Invalid => invalid(spec),
        PortError::OutOfRange => format!("Port „{}“ liegt außerhalb von 1–65535.", shown(spec)),
    };
    if spec.chars().count() > MAX_SPEC_CHARS {
        return Err(invalid(spec));
    }
    let safe = |c: char| c.is_ascii_alphanumeric() || ".:-/".contains(c);
    if !spec.chars().all(safe) {
        return Err(invalid(spec));
    }
    let (address, udp) = match spec.split_once('/') {
        None => (spec, false),
        Some((address, protocol)) if protocol.eq_ignore_ascii_case("tcp") => (address, false),
        Some((address, protocol)) if protocol.eq_ignore_ascii_case("udp") => (address, true),
        Some(_) => return Err(invalid(spec)),
    };
    let parts: Vec<&str> = address.split(':').collect();
    let (ip, host, container) = match parts.as_slice() {
        [single] => {
            let ports = range(single).map_err(fail)?;
            (Ipv4Addr::LOCALHOST, ports, ports)
        }
        [host, container] => (
            Ipv4Addr::LOCALHOST,
            range(host).map_err(fail)?,
            range(container).map_err(fail)?,
        ),
        [ip, host, container] => {
            let ip = ip
                .parse::<Ipv4Addr>()
                .map_err(|_| format!("IP-Adresse in „{}“ ist ungültig (nur IPv4).", shown(spec)))?;
            (
                ip,
                range(host).map_err(fail)?,
                range(container).map_err(fail)?,
            )
        }
        _ => return Err(invalid(spec)),
    };
    if host.1 - host.0 != container.1 - container.0 {
        return Err(format!(
            "Host- und Container-Bereich in „{}“ müssen gleich lang sein.",
            shown(spec)
        ));
    }
    // Spans are equal by now; Docker starts one proxy per published port.
    if host.1 - host.0 >= MAX_RANGE_PORTS {
        return Err(format!(
            "Bereich in „{}“ umfasst mehr als 100 Ports.",
            shown(spec)
        ));
    }
    Ok(Spec {
        ip,
        host,
        container,
        udp,
    })
}

/// One spec in canonical form (trimmed first); German error when it is not Docker's grammar.
pub fn normalize_port(spec: &str) -> Result<String, String> {
    parse(spec.trim()).map(|spec| spec.canonical())
}

/// A whole list: trimmed, blanks dropped, each normalized, order kept; refuses more than
/// [`MAX_PORTS`] entries and any host port two entries of the same protocol would both bind.
pub fn normalize_ports(list: &[String]) -> Result<Vec<String>, String> {
    let entries: Vec<&str> = list
        .iter()
        .map(|spec| spec.trim())
        .filter(|spec| !spec.is_empty())
        .collect();
    if entries.len() > MAX_PORTS {
        return Err(TOO_MANY.into());
    }
    let mut parsed: Vec<Spec> = Vec::with_capacity(entries.len());
    for entry in entries {
        let spec = parse(entry)?;
        if let Some(port) = parsed.iter().find_map(|earlier| earlier.overlap(&spec)) {
            let protocol = if spec.udp { "/udp" } else { "" };
            return Err(format!("Host-Port {port}{protocol} ist doppelt belegt."));
        }
        parsed.push(spec);
    }
    Ok(parsed.iter().map(Spec::canonical).collect())
}

#[cfg(test)]
mod tests;
