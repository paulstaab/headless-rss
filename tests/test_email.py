from src import feed
from src.email import add_credentials, fetch_emails


def test_fetch_emails(mocker):
    """Test fetching emails from the configured mailbox."""
    # given
    add_credentials(
        protocol="imap", server="imap.example.com", port=993, username="user@example.com", password="password123"
    )
    mocker.patch("src.email.imaplib.IMAP4_SSL")
    mock_imap = mocker.Mock()
    mock_imap.search.return_value = ("OK", [b"1 2 3"])
    mock_imap.fetch.side_effect = [
        ("OK", [(b"1", b"Subject: Test Email 1\nFrom: mailing-list1@example.com\nList-Unsubscribe: a1\n\nBody 1")]),
        ("OK", [(b"2", b"Subject: Test Email 2\nFrom: mailing-list1@example.com\nList-Unsubscribe: a1\n\nBody 2")]),
        ("OK", [(b"3", b"Subject: Test Email 3\nFrom: mailing-list2@example.com\nList-Unsubscribe: b2\n\nBody 3")]),
    ]
    mocker.patch("src.email.imaplib.IMAP4_SSL", return_value=mock_imap)

    # when
    fetch_emails()
    feed.update_all()

    # then
    feeds = feed.get_all()
    assert len(feeds) == 2
