//! Usable-address filtering, display URLs and QR codes.
//!
//! Split out of `main.rs` — pure network-presentation logic, no serving.

use std::net::IpAddr;

use colored::Colorize;
use if_addrs::{Interface, get_if_addrs};
use leptos::logging::warn;
use qr_code::QrCode;

/// Interface-name prefixes of virtual container bridges (Docker, libvirt,
/// veth pairs). Their addresses are never reachable from other machines,
/// so they are skipped when expanding wildcard binds for display.
const VIRTUAL_IFACE_PREFIXES: &[&str] = &["docker", "veth", "br-", "virbr"];

/// Unicast scopes with no usable route for LAN clients (ARP/ND link-local).
fn is_link_local(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_unicast_link_local(),
    }
}

/// Addresses worth showing to the user: drops loopback, link-local,
/// multicast and unspecified addresses.
#[must_use]
pub fn is_usable_address(ip: &IpAddr) -> bool {
    !(ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() || is_link_local(ip))
}

/// A served address plus its printable URL.
pub struct DisplayTarget {
    pub ip: IpAddr,
    pub url: String,
}

#[must_use]
pub fn format_url(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(_) => format!("http://{ip}:{port}"),
        IpAddr::V6(_) => format!("http://[{ip}]:{port}"),
    }
}

/// Concrete addresses to show: explicitly configured addresses as-is, plus
/// usable LAN addresses discovered for wildcards (deduped).
pub fn display_targets(interfaces: &[IpAddr], port: u16) -> Vec<DisplayTarget> {
    let (wildcards, explicit): (Vec<IpAddr>, Vec<IpAddr>) =
        interfaces.iter().copied().partition(IpAddr::is_unspecified);

    let mut targets: Vec<DisplayTarget> = explicit
        .into_iter()
        .map(|ip| DisplayTarget {
            ip,
            url: format_url(ip, port),
        })
        .collect();

    // Replace wildcard addresses with usable local interface addresses.
    if !wildcards.is_empty() {
        let want_v4 = wildcards.iter().any(IpAddr::is_ipv4);
        let want_v6 = wildcards.iter().any(IpAddr::is_ipv6);

        match get_if_addrs() {
            Ok(ifaces) => {
                let mut found: Vec<IpAddr> = ifaces
                    .iter()
                    .filter(|iface| {
                        let ip = iface.ip();
                        ((want_v4 && ip.is_ipv4()) || (want_v6 && ip.is_ipv6()))
                            && is_usable_address(&ip)
                            && !VIRTUAL_IFACE_PREFIXES
                                .iter()
                                .any(|prefix| iface.name.starts_with(prefix))
                    })
                    .map(Interface::ip)
                    .collect();
                found.sort_unstable();
                found.dedup();
                found.retain(|ip| !targets.iter().any(|target| target.ip == *ip));
                targets.extend(found.into_iter().map(|ip| DisplayTarget {
                    ip,
                    url: format_url(ip, port),
                }));
            },
            Err(e) => {
                warn!("Failed to list network interfaces, showing configured addresses only: {e}");
            },
        }
    }

    targets
}

pub fn print_qr_codes(targets: &[DisplayTarget]) {
    for target in targets {
        // Loopback/link-local addresses are useless on another device.
        if !is_usable_address(&target.ip) {
            continue;
        }
        match QrCode::new(&target.url) {
            Ok(qr) => {
                println!(
                    "\n QR code for {}:\n{}",
                    target.url.green().bold(),
                    qr.to_string(false, 1)
                );
            },
            // A single bad address must not hide the rest; only a failed
            // bind is fatal.
            Err(e) => warn!("Failed to render QR code for {}: {e}", target.url),
        }
    }
}
