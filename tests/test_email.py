from src import article, feed
from src.email import add_credentials


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
