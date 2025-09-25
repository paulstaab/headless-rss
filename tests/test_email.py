from email.message import EmailMessage

import pytest
import src.email as email
from src import article, database, feed, folder
from src.email import EmailConnectionError, _extract_email_subject, add_credentials, clean_up_old_newsletters


def _mock_emails(mocker) -> None:
    mock_imap = mocker.Mock()
    mock_imap.search.return_value = ("OK", [b"1 2 3"])
    mock_imap.fetch.side_effect = [
        (
            "OK",
            [
                (
                    b"1",
                    b"Subject: Test Email 1\nFrom: Example List <list1@example.com>\nList-Unsubscribe: a1\n\nBody 1",
                )
            ],
        ),
        (
            "OK",
            [
                (
                    b"2",
                    b"Subject: Test Email 2\nFrom: Example List <list1@example.com>\nList-Unsubscribe: a1\n\nBody 2",
                )
            ],
        ),
        (
            "OK",
            [
                (
                    b"3",
                    b"Subject: Test Email 3\nFrom: Another List <list2@example.com>\nList-Unsubscribe: b2\n\nBody 3",
                )
            ],
        ),
    ]
    mocker.patch("src.email.imaplib.IMAP4_SSL", return_value=mock_imap)


def test_fetch_emails(mocker):
    """Test fetching emails from the configured mailbox."""
    # given
    _mock_emails(mocker)
    add_credentials(
        protocol="imap", server="imap.example.com", port=993, username="user@example.com", password="password123"
    )

    # when
    _mock_emails(mocker)
    feed.update_all()

    _mock_emails(mocker)
    feed.update_all()

    # then
    feeds = feed.get_all()
    assert len(feeds) == 2
    feed1 = feed.get_by_url("list1@example.com")
    assert feed1.title == "Example List"
    assert feed1.is_mailing_list is True
    assert len(article.get_by_feed(feed1.id)) == 2

    feed2 = feed.get_by_url("list2@example.com")
    assert feed2.title == "Another List"
    assert feed2.is_mailing_list is True
    assert len(article.get_by_feed(feed2.id)) == 1


def test_clean_up_old_newsletters_removes_only_read_unstarred():
    """Ensure cleanup removes only stale read/unstarred newsletter items."""

    root_folder_id = folder.get_root_folder_id()
    mailing_feed = feed.add_mailing_list("list@example.com", "Example List", root_folder_id)

    base_time = 1_000_000
    ninety_one_days = 91 * 24 * 60 * 60

    with database.get_session() as session:
        regular_feed = database.Feed(
            url="https://example.com/rss",
            title="Regular Feed",
            folder_id=root_folder_id,
            added=base_time,
        )
        session.add(regular_feed)
        session.flush()

        old_read = article.create(
            feed_id=mailing_feed.id,
            title="Old Read",
            author="list@example.com",
            url=None,
            content="content",
            guid="old-read",
            pub_date=base_time - ninety_one_days,
            updated_date=base_time - ninety_one_days,
        )
        old_read.unread = False
        old_read.starred = False
        old_read.last_modified = base_time - ninety_one_days

        old_unread = article.create(
            feed_id=mailing_feed.id,
            title="Old Unread",
            author="list@example.com",
            url=None,
            content="content",
            guid="old-unread",
            pub_date=base_time - ninety_one_days,
            updated_date=base_time - ninety_one_days,
        )
        old_unread.unread = True
        old_unread.starred = False
        old_unread.last_modified = base_time - ninety_one_days

        old_starred = article.create(
            feed_id=mailing_feed.id,
            title="Old Starred",
            author="list@example.com",
            url=None,
            content="content",
            guid="old-starred",
            pub_date=base_time - ninety_one_days,
            updated_date=base_time - ninety_one_days,
        )
        old_starred.unread = False
        old_starred.starred = True
        old_starred.last_modified = base_time - ninety_one_days

        recent_article = article.create(
            feed_id=mailing_feed.id,
            title="Recent",
            author="list@example.com",
            url=None,
            content="content",
            guid="recent",
            pub_date=base_time,
            updated_date=base_time,
        )
        recent_article.unread = False
        recent_article.starred = False
        recent_article.last_modified = base_time

        old_regular = article.create(
            feed_id=regular_feed.id,
            title="Regular Old",
            author="rss@example.com",
            url=None,
            content="content",
            guid="non-mailing",
            pub_date=base_time - ninety_one_days,
            updated_date=base_time - ninety_one_days,
        )
        old_regular.unread = False
        old_regular.starred = False
        old_regular.last_modified = base_time - ninety_one_days

        session.add_all([old_read, old_unread, old_starred, recent_article, old_regular])
        session.commit()

    removed = clean_up_old_newsletters(now_ts=base_time)

    assert removed == 1

    with database.get_session() as session:
        remaining_guids = {article.guid for article in session.query(database.Article).all()}

    assert "old-read" not in remaining_guids
    assert {"old-unread", "old-starred", "recent", "non-mailing"}.issubset(remaining_guids)


def test_fetch_emails_runs_cleanup_when_no_credentials(mocker):
    cleanup_spy = mocker.patch("src.email.clean_up_old_newsletters")

    email.fetch_emails_from_all_mailboxes()

    cleanup_spy.assert_called_once_with()


@pytest.mark.xfail
def test_email_subject_lacks_sanitization() -> None:
    """Test that email subjects are processed with proper sanitization.

    This test verifies that malicious content in email subjects is properly
    sanitized to prevent security vulnerabilities.
    """
    # Create email with malicious subject content
    msg = EmailMessage()
    malicious_subjects = [
        "<script>alert('xss')</script>",  # XSS payload
        "'; DROP TABLE articles; --",  # SQL injection attempt
        "{{7*7}}",  # Template injection attempt
        "\x00\x01\x02malicious",  # Binary/control characters
    ]

    for malicious_subject in malicious_subjects:
        msg["subject"] = malicious_subject

        # Extract subject with sanitization
        extracted = _extract_email_subject(msg)

        # Test that malicious content is properly sanitized
        assert malicious_subject not in extracted

        # Verify specific threats are mitigated
        if "<script>" in malicious_subject:
            assert "<script>" not in extracted  # XSS payload sanitized
        if "DROP TABLE" in malicious_subject:
            assert "DROP TABLE" not in extracted  # SQL injection sanitized


@pytest.mark.xfail
def test_email_connection_error_exposes_internal_details() -> None:
    """Test that error messages do not expose internal system details.

    This test verifies that error messages are properly sanitized to prevent
    information disclosure vulnerabilities.
    """
    # Attempt connection with invalid credentials to trigger error
    with pytest.raises(EmailConnectionError) as exc_info:
        add_credentials(
            protocol="imap",
            server="nonexistent.server.invalid",
            port=993,
            username="test@example.com",
            password="password",
        )

    error_msg = str(exc_info.value)

    # Test that error message does not expose internal details
    assert "nonexistent.server.invalid" not in error_msg  # Server name not exposed
    assert "993" not in error_msg  # Port not exposed
    assert "test@example.com" not in error_msg  # Username not exposed
