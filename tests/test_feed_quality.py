from unittest.mock import patch

from src import database, feed, folder


def test_feed_quality_fulltext_better(feed_server):
    """Test that use_extracted_fulltext is set to True when extracted text is much longer."""
    # given
    root_folder_id = folder.get_root_folder_id()
    feed_url = feed_server.url_for("/atom.xml")

    # Mock extracted text to be significantly longer than the feed content ("Watch out for nasty tricks")
    long_text = "This is a very long text that essentially simulates what a full text extraction would return. " * 5

    # when
    with patch("src.feed.extract_article", return_value=long_text):
        new_feed = feed.add(feed_url, root_folder_id)

    # Reload feed from DB to get updates made by feed.update()
    with database.get_session() as db:
        new_feed = db.get(database.Feed, new_feed.id)

    # then
    assert new_feed is not None
    assert new_feed.use_extracted_fulltext is True
    assert new_feed.last_quality_check is not None


def test_feed_quality_fulltext_worse(feed_server):
    """Test that use_extracted_fulltext is set to False when extracted text is not significantly longer."""
    # given
    root_folder_id = folder.get_root_folder_id()
    feed_url = feed_server.url_for("/rss_2_0.xml")  # Using a different feed just to be clean, though reuse is fine

    # Mock extracted text to be short
    short_text = "Short text."

    # when
    with patch("src.feed.extract_article", return_value=short_text):
        new_feed = feed.add(feed_url, root_folder_id)

    # Reload feed
    with database.get_session() as db:
        new_feed = db.get(database.Feed, new_feed.id)

    # then
    assert new_feed is not None
    assert new_feed.use_extracted_fulltext is False
    assert new_feed.last_quality_check is not None
