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


@pytest.mark.xfail
def test_feed_url_ssrf_vulnerability(mocker) -> None:
    """Test that feed URLs are properly validated to prevent SSRF attacks.
    
    This test verifies that dangerous URLs are blocked to prevent SSRF
    vulnerabilities.
    """
    from src.feed import _parse
    
    # Mock feedparser.parse to avoid actual network requests
    mock_parse = mocker.patch('src.feed.feedparser.parse')
    mock_parse.return_value.bozo = False
    
    # Test dangerous URLs that should be blocked
    dangerous_urls = [
        "file:///etc/passwd",  # Local file access
        "http://localhost:8080/admin",  # Internal service access  
        "http://192.168.1.1/",  # Private network access
        "http://169.254.169.254/latest/meta-data/",  # Cloud metadata service
    ]
    
    for url in dangerous_urls:
        # This call should not process dangerous URLs
        _parse(url)
        # Verify that feedparser.parse was NOT called with the dangerous URL
        mock_parse.assert_not_called()
    
    # Test that safe URLs are still processed
    safe_url = "https://example.com/feed.xml"
    _parse(safe_url)
    mock_parse.assert_called_with(safe_url)
