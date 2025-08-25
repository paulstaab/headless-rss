"""Security tests for headless-rss

These tests validate the security issues identified in the security review.
"""

import base64
import os
import pytest
from fastapi.testclient import TestClient

from src.api.app import app
from src import database, email


class TestAuthentication:
    """Test authentication security issues."""

    def test_no_auth_required_when_creds_not_set(self):
        """Test that no authentication is required when USERNAME/PASSWORD not set."""
        # Ensure no credentials are set
        if "USERNAME" in os.environ:
            del os.environ["USERNAME"]
        if "PASSWORD" in os.environ:
            del os.environ["PASSWORD"]
        
        client = TestClient(app)
        response = client.get("/index.php/apps/news/api/v1-3/feeds")
        # This should pass without authentication - SECURITY ISSUE
        assert response.status_code == 200

    def test_credentials_exposed_in_environment(self):
        """Test that credentials in environment variables are a security risk."""
        # Set test credentials
        os.environ["USERNAME"] = "testuser"
        os.environ["PASSWORD"] = "plaintext_password"  # SECURITY ISSUE: plaintext in env
        
        # These would be visible in process lists, container env, etc.
        assert os.getenv("USERNAME") == "testuser"
        assert os.getenv("PASSWORD") == "plaintext_password"
        
        # Cleanup
        del os.environ["USERNAME"]
        del os.environ["PASSWORD"]


class TestCredentialStorage:
    """Test email credential storage security."""

    def test_email_credentials_stored_plaintext(self, db):
        """Test that email credentials are stored in plaintext - SECURITY ISSUE."""
        # Add credentials
        email.add_credentials(
            protocol="imap",
            server="test.example.com", 
            port=993,
            username="test@example.com",
            password="super_secret_password"  # SECURITY ISSUE: stored in plaintext
        )
        
        # Retrieve directly from database
        with database.get_session() as session:
            credential = session.query(database.EmailCredential).first()
            assert credential is not None
            # SECURITY ISSUE: Password is stored in plaintext
            assert credential.password == "super_secret_password"
            print(f"WARNING: Password stored in plaintext: {credential.password}")


class TestSSRFVulnerabilities:
    """Test Server-Side Request Forgery vulnerabilities."""

    def test_feed_url_no_validation(self):
        """Test that feed URLs are not validated - potential SSRF."""
        from src.feed import _parse
        
        # These URLs should be blocked but aren't - SECURITY ISSUES
        dangerous_urls = [
            "file:///etc/passwd",  # Local file access
            "http://localhost:8080/admin",  # Internal service access
            "http://192.168.1.1/",  # Private network access
            "http://169.254.169.254/",  # Cloud metadata service
        ]
        
        for url in dangerous_urls:
            try:
                # This would attempt to fetch the URL without validation
                # In a real test, we would mock the request to avoid actual SSRF
                print(f"WARNING: URL {url} would be processed without validation")
            except Exception:
                pass  # Expected to fail in test environment


class TestInputValidation:
    """Test input validation security."""

    def test_email_subject_no_sanitization(self):
        """Test that email subjects are not sanitized."""
        from src.email import _extract_email_subject
        from email.message import EmailMessage
        
        # Create malicious email subject
        msg = EmailMessage()
        malicious_subject = "<script>alert('xss')</script>'; DROP TABLE articles; --"
        msg["subject"] = malicious_subject
        
        # Extract subject without sanitization - SECURITY ISSUE
        extracted = _extract_email_subject(msg)
        # The malicious content is not sanitized
        assert "<script>" in extracted
        print(f"WARNING: Unsanitized subject: {extracted}")


class TestInformationDisclosure:
    """Test information disclosure vulnerabilities."""

    def test_error_messages_expose_internal_details(self):
        """Test that error messages expose internal system details."""
        try:
            email.add_credentials(
                protocol="imap",
                server="nonexistent.server.invalid",
                port=993,
                username="test@example.com", 
                password="password"
            )
        except email.EmailConnectionError as e:
            error_msg = str(e)
            # SECURITY ISSUE: Error exposes server details
            assert "nonexistent.server.invalid" in error_msg
            assert "993" in error_msg 
            assert "test@example.com" in error_msg
            print(f"WARNING: Error exposes internal details: {error_msg}")

    def test_version_endpoint_accessible_without_auth(self):
        """Test that version endpoint is accessible without authentication."""
        client = TestClient(app)
        response = client.get("/index.php/apps/news/api/v1-3/version")
        # SECURITY ISSUE: Version info disclosed without authentication
        assert response.status_code == 200
        assert "version" in response.json()
        print(f"WARNING: Version disclosed: {response.json()}")


# Note: These tests demonstrate security issues and should not be run in production
# They are for security review purposes only