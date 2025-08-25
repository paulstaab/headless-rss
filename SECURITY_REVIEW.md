# Security Review Report - headless-rss

**Date:** August 25, 2025  
**Repository:** paulstaab/headless-rss  
**Reviewer:** Security Review Bot  

## Executive Summary

This security review identified several high and medium priority security vulnerabilities in the headless-rss application. The most critical issues relate to:

1. **Plaintext credential storage** - Email credentials stored unencrypted in SQLite database
2. **Weak authentication mechanisms** - Basic Auth over HTTP, credentials in environment variables
3. **Insufficient input validation** - URLs and email content not properly validated/sanitized
4. **Information disclosure** - Sensitive data exposure in logs and error messages
5. **Container security** - Missing security hardening in Docker configuration

## Critical Security Issues

### 🔴 **HIGH SEVERITY**

#### 1. Plaintext Email Credential Storage
**File:** `src/database.py:88-95`, `src/email.py:16-28`  
**Risk:** Credential compromise  

Email passwords are stored in plaintext in the SQLite database:

```python
class EmailCredential(Base):
    __tablename__ = "email_credentials"
    username = Column(String, nullable=False)
    password = Column(String, nullable=False)  # ⚠️ PLAINTEXT PASSWORD
```

**Impact:** If the database file is compromised, all email credentials are immediately exposed.

**Recommendation:** 
- Encrypt passwords using a key derivation function (PBKDF2, bcrypt, scrypt)
- Store only password hashes, not plaintext passwords
- Consider using application-level encryption for the entire credentials table

#### 2. Weak Authentication Implementation
**File:** `src/api/nextcloud_news/v1_3/app.py:16-40`, `src/api/nextcloud_news/v1_2/app.py:16-40`  
**Risk:** Authentication bypass, credential exposure

Multiple authentication weaknesses:

```python
def get_current_username(credentials):
    username = os.getenv("USERNAME")  # ⚠️ Environment variable exposure
    password = os.getenv("PASSWORD")  # ⚠️ Plaintext in env
    if username is None or password is None:
        return  # ⚠️ No authentication if not set
```

**Issues:**
- Credentials stored in environment variables (visible in process lists)
- No authentication when USERNAME/PASSWORD not set
- Basic Auth over HTTP (credentials transmitted in base64, easily decoded)
- No rate limiting on authentication attempts
- No account lockout mechanisms

**Recommendation:**
- Implement proper authentication with hashed passwords
- Enforce HTTPS for all authenticated endpoints
- Add rate limiting and account lockout
- Use secure credential storage (secrets management)

#### 3. Server-Side Request Forgery (SSRF) via Feed URLs
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

#### 4. Command Injection via Email Subject Processing
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

#### 5. Information Disclosure in Error Messages
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

#### 6. Insufficient Input Validation
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

#### 9. Information Disclosure via Version Endpoint
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

#### 7. Container Security Hardening
**File:** `pyproject.toml:7-15`  
**Risk:** Known vulnerabilities in dependencies

**Issues:**
- No automated dependency vulnerability scanning
- Some dependencies may have known CVEs

**Recommendation:**
- Implement automated dependency scanning (Dependabot, Snyk)
- Regular dependency updates
- Pin exact versions for reproducible builds

#### 8. Dependency Security

### Logging and Monitoring
- Implement security event logging
- Add monitoring for suspicious activity
- Log authentication attempts and failures

### Data Protection
- Implement database encryption at rest
- Add data backup encryption
- Consider database file permissions hardening

### Network Security
- Force HTTPS in production
- Implement proper TLS configuration
- Add Content Security Policy headers

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

While headless-rss provides useful functionality, several critical security issues need immediate attention, particularly around credential storage and authentication. The application should not be deployed in production environments without addressing the HIGH severity issues identified in this review.

**Priority Order for Fixes:**
1. Encrypt email credentials in database
2. Implement secure authentication mechanism
3. Add SSRF protection for feed URLs
4. Improve input validation and sanitization
5. Harden container security
6. Implement security monitoring

---

*This review was conducted on the codebase as of commit [latest]. Security is an ongoing process, and regular reviews should be conducted as the codebase evolves.*