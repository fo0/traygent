use super::*;

fn list(specs: &[&str]) -> Vec<String> {
    specs.iter().map(|spec| spec.to_string()).collect()
}

#[test]
fn presets_are_the_eight_spec_ports_in_order() {
    let ports: Vec<u16> = presets().iter().map(|preset| preset.port).collect();
    assert_eq!(ports, [3000, 5173, 4200, 8000, 8080, 5000, 4321, 6006]);
    assert_eq!(presets()[0].label, "Next.js · Node");
    assert_eq!(presets()[3].label, "Django · FastAPI");
    for preset in presets() {
        assert_eq!(
            normalize_port(&preset.port.to_string()).unwrap(),
            format!("127.0.0.1:{0}:{0}", preset.port)
        );
    }
}

#[test]
fn a_bare_port_binds_to_loopback_on_the_same_port() {
    assert_eq!(normalize_port("3000").unwrap(), "127.0.0.1:3000:3000");
    assert_eq!(normalize_port("  5173 ").unwrap(), "127.0.0.1:5173:5173");
    assert_eq!(normalize_port("1").unwrap(), "127.0.0.1:1:1");
    assert_eq!(normalize_port("65535").unwrap(), "127.0.0.1:65535:65535");
    assert_eq!(normalize_port("03000").unwrap(), "127.0.0.1:3000:3000");
}

#[test]
fn host_and_container_keep_their_order_on_loopback() {
    assert_eq!(normalize_port("8080:80").unwrap(), "127.0.0.1:8080:80");
}

#[test]
fn an_explicit_ip_is_kept() {
    assert_eq!(
        normalize_port("0.0.0.0:8080:80").unwrap(),
        "0.0.0.0:8080:80"
    );
    assert_eq!(
        normalize_port("192.168.1.20:3000:3000").unwrap(),
        "192.168.1.20:3000:3000"
    );
    assert_eq!(
        normalize_port("127.0.0.1:3000:3000").unwrap(),
        "127.0.0.1:3000:3000"
    );
}

#[test]
fn the_protocol_is_tcp_by_default_and_udp_is_kept() {
    assert_eq!(normalize_port("3000/tcp").unwrap(), "127.0.0.1:3000:3000");
    assert_eq!(normalize_port("3000/TCP").unwrap(), "127.0.0.1:3000:3000");
    assert_eq!(
        normalize_port("0.0.0.0:8080:80/udp").unwrap(),
        "0.0.0.0:8080:80/udp"
    );
    assert_eq!(normalize_port("53:53/UDP").unwrap(), "127.0.0.1:53:53/udp");
    for spec in ["3000/sctp", "3000/", "3000/udp/tcp", "/udp"] {
        assert_eq!(
            normalize_port(spec).unwrap_err(),
            format!("Port „{spec}“ ist ungültig.")
        );
    }
}

#[test]
fn ranges_need_equal_length_and_ascending_bounds() {
    assert_eq!(
        normalize_port("3000-3005").unwrap(),
        "127.0.0.1:3000-3005:3000-3005"
    );
    assert_eq!(
        normalize_port("9000-9002:3000-3002/udp").unwrap(),
        "127.0.0.1:9000-9002:3000-3002/udp"
    );
    assert_eq!(normalize_port("3000-3000").unwrap(), "127.0.0.1:3000:3000");
    assert_eq!(
        normalize_port("8000-8001:80").unwrap_err(),
        "Host- und Container-Bereich in „8000-8001:80“ müssen gleich lang sein."
    );
    assert!(normalize_port("8000:80-81").is_err());
    assert_eq!(
        normalize_port("3005-3000").unwrap_err(),
        "Port „3005-3000“ ist ungültig."
    );
    for spec in ["3000-", "-3000", "3000--3001", "3000-3001-3002"] {
        assert!(normalize_port(spec).is_err(), "{spec}");
    }
}

#[test]
fn ports_outside_the_valid_range_are_refused() {
    for spec in ["0", "65536", "70000", "123456", "0:80", "8080:0", "1-65536"] {
        assert_eq!(
            normalize_port(spec).unwrap_err(),
            format!("Port „{spec}“ liegt außerhalb von 1–65535."),
            "{spec}"
        );
    }
}

#[test]
fn only_ipv4_addresses_are_accepted() {
    for spec in [
        "localhost:3000:3000",
        "256.0.0.1:3000:3000",
        "1.2.3:3000:3000",
        "01.2.3.4:3000:3000",
    ] {
        assert_eq!(
            normalize_port(spec).unwrap_err(),
            format!("IP-Adresse in „{spec}“ ist ungültig (nur IPv4)."),
            "{spec}"
        );
    }
    for spec in ["[::1]:3000:3000", "::1:3000:3000", "fe80::1:80"] {
        assert!(normalize_port(spec).is_err(), "{spec}");
    }
}

#[test]
fn malformed_specs_are_refused() {
    for spec in [
        "",
        "abc",
        ":3000",
        "3000:",
        "127.0.0.1::3000",
        "1.2.3.4:3000",
        "a:b:c:d",
        "3000:80:",
        "3.5",
        "+3000",
        "3e3",
    ] {
        assert!(normalize_port(spec).is_err(), "{spec}");
    }
    assert_eq!(
        normalize_port("abc").unwrap_err(),
        "Port „abc“ ist ungültig."
    );
}

#[test]
fn injection_attempts_never_pass() {
    for spec in [
        "3000 --privileged",
        "-p",
        "--privileged",
        "-3000",
        "3000;calc",
        "3000&calc",
        "3000|calc",
        "3000 3001",
        "3000\t3001",
        "3000\n--privileged",
        "3000'",
        "3000\"",
        "$(calc)",
        "3000`",
        "\u{2019}3000",
        "3000:80 -v C:\\:/host",
        "-v=C:\\:/host",
    ] {
        assert!(normalize_port(spec).is_err(), "{spec:?}");
    }
}

#[test]
fn errors_repeat_a_short_clean_excerpt() {
    let long = "x".repeat(100);
    assert_eq!(
        normalize_port(&long).unwrap_err(),
        format!("Port „{}…“ ist ungültig.", "x".repeat(32))
    );
    assert_eq!(
        normalize_port("30\u{7}00").unwrap_err(),
        "Port „3000“ ist ungültig."
    );
}

#[test]
fn lists_are_trimmed_and_normalized_in_order() {
    assert_eq!(
        normalize_ports(&list(&[
            " 3000 ",
            "",
            "  ",
            "0.0.0.0:8080:80/udp",
            "5173/tcp"
        ]))
        .unwrap(),
        list(&[
            "127.0.0.1:3000:3000",
            "0.0.0.0:8080:80/udp",
            "127.0.0.1:5173:5173"
        ])
    );
    assert_eq!(normalize_ports(&[]).unwrap(), Vec::<String>::new());
    assert_eq!(
        normalize_ports(&list(&["3000", "abc"])).unwrap_err(),
        "Port „abc“ ist ungültig."
    );
}

#[test]
fn duplicate_host_ports_are_refused_per_protocol() {
    let refused = |specs: &[&str]| normalize_ports(&list(specs)).unwrap_err();
    assert_eq!(
        refused(&["3000", "3000"]),
        "Host-Port 3000 ist doppelt belegt."
    );
    assert_eq!(
        refused(&["3000", "127.0.0.1:3000:3000"]),
        "Host-Port 3000 ist doppelt belegt."
    );
    // Same host port, other container port: still one binding on the host.
    assert_eq!(
        refused(&["3000:80", "3000:81"]),
        "Host-Port 3000 ist doppelt belegt."
    );
    // IP-agnostic: 0.0.0.0 covers loopback, two IPs are refused too.
    assert_eq!(
        refused(&["0.0.0.0:8080:80", "8080"]),
        "Host-Port 8080 ist doppelt belegt."
    );
    assert_eq!(
        refused(&["192.168.1.2:8080:80", "10.0.0.2:8080:80"]),
        "Host-Port 8080 ist doppelt belegt."
    );
    assert_eq!(
        refused(&["53/udp", "53:54/udp"]),
        "Host-Port 53/udp ist doppelt belegt."
    );
    // Same port, other protocol, and different container-only collisions are fine.
    assert!(normalize_ports(&list(&["53", "53/udp"])).is_ok());
    assert!(normalize_ports(&list(&["8080:80", "8081:80"])).is_ok());
}

#[test]
fn overlapping_ranges_name_the_first_shared_host_port() {
    let refused = |specs: &[&str]| normalize_ports(&list(specs)).unwrap_err();
    assert_eq!(
        refused(&["3000-3005", "3003"]),
        "Host-Port 3003 ist doppelt belegt."
    );
    assert_eq!(
        refused(&["3004-3008", "3000-3005"]),
        "Host-Port 3004 ist doppelt belegt."
    );
    assert!(normalize_ports(&list(&["3000-3005", "3006-3010"])).is_ok());
}

#[test]
fn at_most_twenty_entries_count_after_dropping_blanks() {
    let twenty: Vec<String> = (3000..3020).map(|port| port.to_string()).collect();
    assert_eq!(normalize_ports(&twenty).unwrap().len(), 20);
    let mut padded = twenty.clone();
    padded.extend(["".to_string(), " ".to_string()]);
    assert_eq!(normalize_ports(&padded).unwrap().len(), 20);
    let mut over = twenty;
    over.push("4000".into());
    assert_eq!(normalize_ports(&over).unwrap_err(), TOO_MANY);
}

#[test]
fn canonical_output_is_stable() {
    for spec in [
        "3000",
        "8080:80",
        "0.0.0.0:8080:80/udp",
        "3000-3005",
        "10.1.2.3:9000-9001:80-81",
    ] {
        let once = normalize_port(spec).unwrap();
        assert_eq!(normalize_port(&once).unwrap(), once);
        assert!(once
            .chars()
            .all(|c| c.is_ascii_digit() || ".:-/udp".contains(c)));
        assert!(!once.starts_with('-'));
    }
}

#[test]
fn ranges_span_at_most_one_hundred_ports() {
    assert_eq!(
        normalize_port("3000-3099").unwrap(),
        "127.0.0.1:3000-3099:3000-3099"
    );
    assert_eq!(
        normalize_port("1.2.3.4:9000-9099:3000-3099/udp").unwrap(),
        "1.2.3.4:9000-9099:3000-3099/udp"
    );
    for spec in ["3000-3100", "1-65535", "9000-9100:3000-3100"] {
        assert_eq!(
            normalize_port(spec).unwrap_err(),
            format!("Bereich in „{spec}“ umfasst mehr als 100 Ports."),
            "{spec}"
        );
    }
    let long = "255.255.255.255:1-65535:1-65535/udp";
    assert_eq!(
        normalize_port(long).unwrap_err(),
        "Bereich in „255.255.255.255:1-65535:1-65535/…“ umfasst mehr als 100 Ports."
    );
}

/// One entry of fixtures/port-vectors.json: normalized form or exact German error.
#[derive(serde::Deserialize)]
struct PortVector {
    input: String,
    normalized: Option<String>,
    error: Option<String>,
}

#[derive(serde::Deserialize)]
struct ListVector {
    list: Vec<String>,
    normalized: Option<Vec<String>>,
    error: Option<String>,
}

#[derive(serde::Deserialize)]
struct Vectors {
    ports: Vec<PortVector>,
    lists: Vec<ListVector>,
}

/// The vectors src/lib/ports.test.ts checks too: both sides refuse and normalize alike.
#[test]
fn shared_vectors_match_the_panel() {
    let vectors: Vectors =
        serde_json::from_str(include_str!("../../../fixtures/port-vectors.json")).unwrap();
    assert!(vectors.ports.len() >= 40);
    for vector in &vectors.ports {
        let expected = match (&vector.normalized, &vector.error) {
            (Some(normalized), None) => Ok(normalized.clone()),
            (None, Some(error)) => Err(error.clone()),
            _ => panic!("{:?}: exactly one of normalized and error", vector.input),
        };
        assert_eq!(
            normalize_port(&vector.input),
            expected,
            "{:?}",
            vector.input
        );
    }
    for vector in &vectors.lists {
        let expected = match (&vector.normalized, &vector.error) {
            (Some(normalized), None) => Ok(normalized.clone()),
            (None, Some(error)) => Err(error.clone()),
            _ => panic!("{:?}: exactly one of normalized and error", vector.list),
        };
        assert_eq!(normalize_ports(&vector.list), expected, "{:?}", vector.list);
    }
}
