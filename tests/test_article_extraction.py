from src.content import extract_article


def test_extract_article_simple(feed_server):
    url = feed_server.url_for("/simple_article.html")
    text = extract_article(url)
    assert text is not None
    assert "main content of the article" in text
    assert "Second paragraph" in text
    assert "Footer text" not in text  # Should not extract footer
