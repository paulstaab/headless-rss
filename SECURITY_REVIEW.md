# Security Review Report - headless-rss

**Date:** August 25, 2025  
**Repository:** paulstaab/headless-rss  
**Reviewer:** Security Review Bot  

## Executive Summary

This security review identified several high and medium priority security vulnerabilities in the headless-rss application. The most critical issues relate to:

1. **Insufficient input validation** - URLs and email content not properly validated/sanitized
2. **Information disclosure** - Sensitive data exposure in logs and error messages
3. **Container security** - Missing security hardening in Docker configuration

## Critical Security Issues

### 🔴 **HIGH SEVERITY**

#### 1. Server-Side Request Forgery (SSRF) via Feed URLs
**File:** `src/feed.py:47`, `src/feed.py:218-225`  
**Risk:** Internal network access, data exfiltration

```python
def _parse(url: str) -> feedparser.FeedParserDict:
    parsed_feed = feedparser.parse(url)  # ⚠️ No URL validation
```

**Issues:**
- No validation of feed URLs before making requests
- Can access internal network resources (localhost, RFC 1918 addresses)
- Can access file:// URLs potentially exposing local files
- No timeout/size limits on feed requests

**Recommendation:**
- Implement URL allowlist/denylist
- Block internal/private IP ranges
- Restrict URL schemes to http/https only
- Add request timeouts and size limits

### 🟡 **MEDIUM SEVERITY**

#### 2. Command Injection via Email Subject Processing
**File:** `src/email.py:165-175`  
**Risk:** Potential command injection

```python
def _extract_email_subject(msg) -> str:
    raw_subject = msg["subject"]
    decoded_subject_parts = decode_header(raw_subject)  # ⚠️ No validation
```

**Issues:**
- Email subjects used directly without sanitization
- Could contain malicious content affecting downstream processing
- No length limits on email subjects

**Recommendation:**
- Sanitize and validate email subject content
- Implement length limits
- Escape special characters before database storage

#### 3. Information Disclosure in Error Messages
**File:** `src/email.py:24-26`, `src/feed.py:89-92`  
**Risk:** Sensitive information exposure

```python
raise EmailConnectionError(
    f"Failed to connect to mailbox at {server}:{port} for user {username}: {e}"
)  # ⚠️ Exposes internal details
```

**Issues:**
- Detailed error messages expose internal system information
- Database connection errors may reveal schema details
- IMAP connection errors expose server configurations

**Recommendation:**
- Implement generic error messages for users
- Log detailed errors securely for debugging
- Avoid exposing internal system details

#### 4. Insufficient Input Validation
**File:** `src/api/nextcloud_news/v1_3/feed.py`, `src/api/nextcloud_news/v1_2/feed.py`  
**Risk:** Data injection, XSS

**Issues:**
- No validation on feed URLs in API endpoints
- No size limits on request payloads
- Missing validation on folder names and other user inputs

**Recommendation:**
- Implement comprehensive input validation
- Add payload size limits
- Validate and sanitize all user inputs

#### 5. Information Disclosure via Version Endpoint
**File:** `src/api/nextcloud_news/v1_3/version.py:13-16`, `src/api/nextcloud_news/v1_2/version.py:13-16`  
**Risk:** Information disclosure

**Issues:**
- Version endpoint accessible without authentication
- May reveal application versions to attackers
- Aids in targeted attacks against known vulnerabilities

**Recommendation:**
- Require authentication for version endpoint
- Consider removing version disclosure entirely
- If needed, provide minimal version information
**File:** `Dockerfile:14-21`, `docker/entrypoint:1-14`  
**Risk:** Privilege escalation

**Issues:**
- Container runs as root by default (not enforcing --user flag)
- No security profiles (AppArmor/SELinux)
- Directory permissions could be more restrictive

**Recommendation:**
- Enforce non-root user in container
- Add security profiles
- Use least-privilege file permissions

### 🟢 **LOW SEVERITY**

#### 6. Container Security Hardening
#### 7. Dependency Security
**File:** `pyproject.toml:7-15`  
**Risk:** Known vulnerabilities in dependencies

**Issues:**
- Regular dependency updates are managed by Dependabot (enabled and active)
- Some dependencies may still have known CVEs between update cycles

**Recommendation:**
- Continue using Dependabot for automated dependency updates
- Monitor security advisories for critical vulnerabilities
- Pin exact versions for reproducible builds

### Logging and Monitoring
- Implement security event logging
- Add monitoring for suspicious activity
- Log authentication attempts and failures

### Data Protection
- Implement database encryption at rest
- Add data backup encryption
- Consider database file permissions hardening

### Rate Limiting
- Implement API rate limiting
- Add feed update frequency limits
- Prevent abuse of email fetching

## Testing Recommendations

Add security-focused tests:
- Authentication bypass attempts
- Input validation tests
- SSRF protection tests
- SQL injection tests (though SQLAlchemy ORM provides good protection)

**Note:** Security test examples have been provided in `security_tests.py` which demonstrates several of the vulnerabilities identified in this review. These tests should NOT be run in production environments.

## Additional Security Recommendations

For production deployments, consider:
- GDPR compliance for email processing
- Data retention policies
- Audit logging requirements
- Incident response procedures

## Conclusion

While headless-rss provides useful functionality, several security issues need attention, particularly around SSRF protection and input validation. The application should not be deployed in production environments without addressing the HIGH severity issues identified in this review.

**Priority Order for Fixes:**
1. Add SSRF protection for feed URLs
2. Improve input validation and sanitization
3. Harden container security
4. Implement security monitoring

---

*This review was conducted on the codebase as of commit 175baef5df09eb38b8863e9c65ca84a4cab85b8d. Security is an ongoing process, and regular reviews should be conducted as the codebase evolves.*