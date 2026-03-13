use std::net::IpAddr;
use std::str::FromStr;

use anyhow::{Context, Result};
use tokio::net::lookup_host;

/// Validates a remote URL against SSRF constraints used by feed operations.
///
/// Rules:
/// - Only `http` and `https` schemes are allowed.
/// - Loopback/private/link-local/unspecified/multicast/metadata targets are blocked.
/// - `localhost` is only allowed in testing mode.
pub async fn validate_remote_url(url: &str, allow_localhost: bool) -> Result<()> {
    let parsed = reqwest::Url::parse(url)
        .with_context(|| "URL scheme '' is not allowed. Only http and https are permitted.")?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        anyhow::bail!(
            "URL scheme '{}' is not allowed. Only http and https are permitted.",
            parsed.scheme()
        );
    }

    let Some(hostname) = parsed.host_str() else {
        anyhow::bail!("URL must have a valid hostname.");
    };

    if !allow_localhost && matches!(hostname, "localhost" | "127.0.0.1" | "::1") {
        anyhow::bail!("Access to localhost is not allowed.");
    }

    if let Ok(ip) = IpAddr::from_str(hostname) {
        validate_ip_address(ip, allow_localhost)?;
    }

    let lookup_port = parsed.port_or_known_default().unwrap_or(80);
    if let Ok(addrs) = lookup_host((hostname, lookup_port)).await {
        for addr in addrs {
            validate_ip_address(addr.ip(), allow_localhost)?;
        }
    }

    Ok(())
}

fn validate_ip_address(ip: IpAddr, allow_localhost: bool) -> Result<()> {
    let is_private = match ip {
        IpAddr::V4(v4) => v4.is_private(),
        IpAddr::V6(v6) => v6.is_unique_local(),
    };
    let is_link_local = match ip {
        IpAddr::V4(v4) => v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_unicast_link_local(),
    };

    if !allow_localhost && ip.is_loopback() {
        anyhow::bail!("Access to loopback address {ip} is not allowed.");
    }
    if is_private && !ip.is_loopback() {
        anyhow::bail!("Access to private address {ip} is not allowed.");
    }
    if is_link_local {
        anyhow::bail!("Access to link-local address {ip} is not allowed.");
    }
    if ip.is_unspecified() {
        anyhow::bail!("Access to unspecified address {ip} is not allowed.");
    }
    if ip.is_multicast() {
        anyhow::bail!("Access to multicast address {ip} is not allowed.");
    }
    if ip == IpAddr::from([169, 254, 169, 254]) {
        anyhow::bail!("Access to cloud metadata service is not allowed.");
    }

    Ok(())
}
