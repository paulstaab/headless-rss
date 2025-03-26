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
