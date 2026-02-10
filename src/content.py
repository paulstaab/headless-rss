import ipaddress
import json
import logging
import re
import socket
from urllib.parse import urlparse

import trafilatura
from openai import OpenAI

from src.options import Options

logger = logging.getLogger(__name__)

OPENAI_TIMEOUT_SECONDS = 10
ARTICLE_MAX_CHARS = 8000


class SSRFProtectionError(Exception):
    """Raised when a URL is blocked due to SSRF protection."""


def strip_html(text: str) -> str:
    return re.sub(r"<[^>]+>", " ", text)


def normalize_text(text: str | None) -> str:
    if not text:
        return ""
    stripped = strip_html(text)
    collapsed = re.sub(r"\s+", " ", stripped)
    return collapsed.strip().lower()


def extract_article(url: str, text_only: bool = False) -> str | None:
    if not url:
        return None

    try:
        validate_url(url)
        downloaded = trafilatura.fetch_url(url)
    except Exception as exc:
        logger.warning(f"Failed to fetch article url for extraction: {exc}")
        return None

    if not downloaded:
        return None

    try:
        if text_only:
            extracted = trafilatura.extract(downloaded, output_format="text")
        else:
            extracted = trafilatura.extract(
                downloaded,
                output_format="html",
                include_comments=True,
                include_tables=True,
                include_images=True,
            )
    except Exception as exc:
        logger.warning(f"Failed to extract article text: {exc}")
        return None

    extracted = extracted or ""
    extracted = extracted.strip()

    if not extracted:
        return None

    return extracted


def summarize_article_with_llm(article_text: str) -> str | None:
    if not _llm_enabled():
        return None

    normalized_text = strip_html(article_text)
    trimmed_text = _trim_article_text(normalized_text)
    if not trimmed_text:
        return None

    try:
        response_obj = _call_openai_summary_api(trimmed_text)
    except Exception as exc:
        logger.warning(f"LLM article summarization failed: {exc}")
        return None

    response_text = _extract_openai_response_text(response_obj)
    if not response_text:
        return None

    try:
        parsed = json.loads(response_text)
    except json.JSONDecodeError:
        logger.warning("LLM summary response was not valid JSON")
        return None

    summary = parsed.get("summary") or ""
    summary = summary.strip()

    if not summary:
        return None

    return summary + " (AI generated)"


def _llm_enabled() -> bool:
    return Options.get().llm_enabled


def _trim_article_text(content: str) -> str:
    return (content or "")[:ARTICLE_MAX_CHARS]


def _call_openai_summary_api(article_text: str):
    api_key = Options.get().openai_api_key
    if not api_key:
        raise ValueError("OPENAI_API_KEY is not set")

    payload = _build_openai_summary_payload(article_text)
    client = OpenAI(api_key=api_key, timeout=OPENAI_TIMEOUT_SECONDS)
    return client.chat.completions.create(**payload)


def _call_openai_summary_quality_api(article_text: str, summary: str):
    api_key = Options.get().openai_api_key
    if not api_key:
        raise ValueError("OPENAI_API_KEY is not set")

    payload = _build_openai_summary_quality_payload(article_text, summary)
    client = OpenAI(api_key=api_key, timeout=OPENAI_TIMEOUT_SECONDS)
    return client.chat.completions.create(**payload)


def _build_openai_summary_payload(article_text: str) -> dict:
    options = Options.get()
    instructions = (
        "Summarize the article clearly and concisely. Return 3-6 sentences, no bullets, no headings, plain text only."
    )

    user_prompt = "Article:\n" + article_text

    schema = {
        "name": "article_summary",
        "schema": {
            "type": "object",
            "properties": {
                "summary": {"type": "string"},
            },
            "required": ["summary"],
            "additionalProperties": False,
        },
    }

    return {
        "model": options.openai_model,
        "messages": [
            {"role": "system", "content": instructions},
            {"role": "user", "content": user_prompt},
        ],
        "response_format": {"type": "json_schema", "json_schema": schema},
    }


def _build_openai_summary_quality_payload(article_text: str, summary: str) -> dict:
    options = Options.get()
    instructions = (
        "You are evaluating whether a summary is a good standalone summary of an article. "
        "Answer with is_good=true if it captures the main points and is not just a lead-in."
    )

    user_prompt = "Article:\n" + article_text + "\n\nSummary:\n" + summary

    schema = {
        "name": "summary_quality",
        "schema": {
            "type": "object",
            "properties": {
                "is_good": {"type": "boolean"},
            },
            "required": ["is_good"],
            "additionalProperties": False,
        },
    }

    return {
        "model": options.openai_model,
        "messages": [
            {"role": "system", "content": instructions},
            {"role": "user", "content": user_prompt},
        ],
        "response_format": {"type": "json_schema", "json_schema": schema},
    }


def _extract_openai_response_text(response) -> str | None:
    if response is None:
        return None

    choices = getattr(response, "choices", [])
    if choices and len(choices) > 0:
        message = getattr(choices[0], "message", None)
        if message:
            return getattr(message, "content", None)

    return None


def _validate_url_scheme(parsed_url) -> None:
    """Validate that the URL uses an allowed scheme."""
    if parsed_url.scheme not in ("http", "https"):
        raise SSRFProtectionError(
            f"URL scheme '{parsed_url.scheme}' is not allowed. Only http and https are permitted."
        )


def _validate_hostname(hostname: str | None, allow_localhost: bool) -> None:
    """Validate that the hostname is not blocked."""
    if not hostname:
        raise SSRFProtectionError("URL must have a valid hostname.")

    # Block localhost variants (unless explicitly allowed)
    if not allow_localhost and hostname.lower() in ("localhost", "127.0.0.1", "::1"):
        raise SSRFProtectionError("Access to localhost is not allowed.")


def _validate_ip_address(ip: ipaddress.IPv4Address | ipaddress.IPv6Address, ip_str: str, allow_localhost: bool) -> None:
    """Validate that an IP address is not in blocked ranges."""
    # Block loopback addresses (unless explicitly allowed)
    if not allow_localhost and ip.is_loopback:
        raise SSRFProtectionError(f"Access to loopback address {ip} is not allowed.")

    # Block private addresses (RFC 1918) - but skip if already handled as loopback
    if ip.is_private and not ip.is_loopback:
        raise SSRFProtectionError(f"Access to private address {ip} is not allowed.")

    # Block link-local addresses
    if ip.is_link_local:
        raise SSRFProtectionError(f"Access to link-local address {ip} is not allowed.")

    # Block unspecified addresses (0.0.0.0, ::)
    if ip.is_unspecified:
        raise SSRFProtectionError(f"Access to unspecified address {ip} is not allowed.")

    # Block multicast addresses
    if ip.is_multicast:
        raise SSRFProtectionError(f"Access to multicast address {ip} is not allowed.")

    # Additional check for cloud metadata service (AWS, GCP, Azure common endpoint)
    if ip_str == "169.254.169.254":
        raise SSRFProtectionError("Access to cloud metadata service is not allowed.")


def validate_url(url: str, allow_localhost: bool | None = None) -> None:
    """Validate that a feed URL is safe to access (SSRF protection).

    This function blocks URLs that could be used for Server-Side Request Forgery (SSRF) attacks:
    - Non-HTTP/HTTPS schemes (file://, ftp://, etc.)
    - Localhost and loopback addresses (127.x.x.x, ::1, localhost)
    - Private network addresses (RFC 1918: 10.x.x.x, 172.16-31.x.x, 192.168.x.x)
    - Link-local addresses (169.254.x.x, fe80::/10)
    - Cloud metadata services (169.254.169.254)

    :param url: The URL to validate.
    :param allow_localhost: If True, allows localhost/loopback addresses. If None, auto-detects testing mode.
    :raises SSRFProtectionError: If the URL is blocked for security reasons.
    """
    # Auto-detect testing mode if not explicitly specified
    if allow_localhost is None:
        allow_localhost = Options.get().testing_mode

    parsed_url = urlparse(url)
    _validate_url_scheme(parsed_url)

    hostname = parsed_url.hostname
    _validate_hostname(hostname, allow_localhost)

    # Now we know hostname is not None due to validation
    assert hostname is not None

    # Try to resolve hostname to IP and check if it's in blocked ranges
    try:
        # Get all IP addresses for this hostname
        addr_info = socket.getaddrinfo(hostname, None)
        for _family, _type, _proto, _canonname, sockaddr in addr_info:
            ip_str = str(sockaddr[0])  # Ensure it's a string
            try:
                ip = ipaddress.ip_address(ip_str)
                _validate_ip_address(ip, ip_str, allow_localhost)
            except ValueError:
                # If it's not a valid IP address, continue (could be IPv6 or malformed)
                continue

    except socket.gaierror:
        # DNS resolution failed - this is likely a real domain issue, let it proceed
        # The actual HTTP request will fail with a proper error
        pass
