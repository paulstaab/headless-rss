import pytest
from src import article, feed, folder


# when adding new examples, also add them to the feed_server in conftest.py
@pytest.fixture(params=["/atom.xml", "/github_releases.atom", "/feed_without_ids.xml", "/rss_2_0.xml", "/atom_1_0.xml"])
def feed_url(request, feed_server):
    return feed_server.url_for(request.param)


def test_feed_parsing(feed_url: str) -> None:
    # given
    root_folder_id = folder.get_root_folder_id()
    # when
    feed.add(feed_url, root_folder_id)
    # then
    new_feed = feed.get_all()[0]
    assert new_feed.url == feed_url
    articles = article.get_by_feed(new_feed.id)
    assert len(articles) > 0


def test_feed_url_ssrf_vulnerability(mocker) -> None:
    """Test that feed URLs are processed without SSRF validation.
    
    This demonstrates the SSRF vulnerability where dangerous URLs
    would be processed without validation.
    """
    from src.feed import _parse
    
    # Mock feedparser.parse to avoid actual network requests
    mock_parse = mocker.patch('src.feed.feedparser.parse')
    mock_parse.return_value.bozo = False
    
    # Test dangerous URLs that should be blocked but aren't
    dangerous_urls = [
        "file:///etc/passwd",  # Local file access
        "http://localhost:8080/admin",  # Internal service access  
        "http://192.168.1.1/",  # Private network access
        "http://169.254.169.254/latest/meta-data/",  # Cloud metadata service
    ]
    
    for url in dangerous_urls:
        # This call would process the URL without SSRF validation
        _parse(url)
        # Verify that feedparser.parse was called with the dangerous URL
        mock_parse.assert_called_with(url)
    
    # The vulnerability exists because no URL validation occurs before calling feedparser.parse
