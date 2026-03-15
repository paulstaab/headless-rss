use std::net::IpAddr;
use std::str::FromStr;

use anyhow::{Context, Result};
use reqwest::header::LOCATION;
use tokio::net::lookup_host;

const MAX_REDIRECTS: usize = 3;

#[derive(Debug)]
pub enum SafeGetError {
    Validation(anyhow::Error),
    Request(anyhow::Error),
}

impl SafeGetError {
    /// Converts the wrapped error into an anyhow error for callers that do not
    /// need to distinguish validation failures from transport failures.
    pub fn into_anyhow(self) -> anyhow::Error {
        match self {
            Self::Validation(error) | Self::Request(error) => error,
        }
    }
}

/// Validates a remote URL against SSRF constraints used by feed operations.
///
/// Rules:
/// - Only `http` and `https` schemes are allowed.
/// - Loopback/private/link-local/unspecified/multicast/metadata targets are blocked.
/// - `localhost` is only allowed in testing mode.
pub async fn validate_remote_url(url: &str, allow_localhost: bool) -> Result<reqwest::Url> {
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
        return Ok(parsed);
    }

    let lookup_port = parsed.port_or_known_default().unwrap_or(80);
    let addrs = lookup_host((hostname, lookup_port))
        .await
        .with_context(|| {
            format!("failed to resolve hostname `{hostname}` during SSRF validation")
        })?;

    for addr in addrs {
        validate_ip_address(addr.ip(), allow_localhost)?;
    }

    Ok(parsed)
}

/// Sends a GET request while validating each redirect hop against the SSRF
/// policy before the client follows it.
pub async fn get_with_safe_redirects(
    client: &reqwest::Client,
    url: &str,
    allow_localhost: bool,
) -> std::result::Result<reqwest::Response, SafeGetError> {
    let mut current_url = validate_remote_url(url, allow_localhost)
        .await
        .map_err(SafeGetError::Validation)?;

    for redirect_count in 0..=MAX_REDIRECTS {
        let response = client
            .get(current_url.clone())
            .send()
            .await
            .map_err(|error| SafeGetError::Request(error.into()))?;

        let status = response.status();
        let is_follow_redirect = matches!(
            status,
            reqwest::StatusCode::MOVED_PERMANENTLY
                | reqwest::StatusCode::FOUND
                | reqwest::StatusCode::SEE_OTHER
                | reqwest::StatusCode::TEMPORARY_REDIRECT
                | reqwest::StatusCode::PERMANENT_REDIRECT
        );

        if !is_follow_redirect {
            return Ok(response);
        }

        if redirect_count == MAX_REDIRECTS {
            return Err(SafeGetError::Request(anyhow::anyhow!(
                "stopped after {MAX_REDIRECTS} redirects"
            )));
        }

        let location = response
            .headers()
            .get(LOCATION)
            .ok_or_else(|| {
                SafeGetError::Request(anyhow::anyhow!("redirect response missing Location header"))
            })?
            .to_str()
            .map_err(|error| {
                SafeGetError::Request(
                    anyhow::Error::new(error)
                        .context("redirect Location header is not valid UTF-8"),
                )
            })?;

        current_url = validate_redirect_target(response.url(), location, allow_localhost)
            .await
            .map_err(SafeGetError::Validation)?;
    }

    Err(SafeGetError::Request(anyhow::anyhow!(
        "redirect handling terminated unexpectedly"
    )))
}

/// Resolves a redirect target relative to the current URL and validates the
/// resulting target before it is fetched.
pub async fn validate_redirect_target(
    current_url: &reqwest::Url,
    location: &str,
    allow_localhost: bool,
) -> Result<reqwest::Url> {
    let redirect_url = current_url
        .join(location)
        .with_context(|| format!("invalid redirect target `{location}`"))?;
    validate_remote_url(redirect_url.as_str(), allow_localhost).await?;
    Ok(redirect_url)
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

#[cfg(test)]
mod tests {
    use super::{validate_redirect_target, validate_remote_url};

    #[tokio::test]
    async fn validate_remote_url_rejects_hostname_when_dns_lookup_fails() {
        let error = validate_remote_url("https://nonexistent.invalid/feed.xml", false)
            .await
            .unwrap_err();

        assert!(
            error.to_string().contains(
                "failed to resolve hostname `nonexistent.invalid` during SSRF validation"
            )
        );
    }

    #[tokio::test]
    async fn validate_redirect_target_blocks_localhost_hop() {
        let current_url = reqwest::Url::parse("https://example.com/feed.xml").unwrap();

        let error = validate_redirect_target(&current_url, "http://127.0.0.1/admin", false)
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), "Access to localhost is not allowed.");
    }

    #[tokio::test]
    async fn validate_redirect_target_accepts_relative_public_hop() {
        let current_url = reqwest::Url::parse("https://example.com/feed.xml").unwrap();

        let redirect_url = validate_redirect_target(&current_url, "/next.xml", false)
            .await
            .unwrap();

        assert_eq!(redirect_url.as_str(), "https://example.com/next.xml");
    }
}
