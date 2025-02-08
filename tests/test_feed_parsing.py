import pytest
from src import database
from src.feed import create, update


@pytest.fixture(params=["/atom.xml", "/github_releases.atom"])
def feed_url(request, feed_server):
    return feed_server.url_for(request.param)


def test_feed_parsing(feed_url):
    # when
    with database.get_session() as db:
        feed = create(feed_url)
        db.add(feed)
        db.commit()
        db.refresh(feed)
        update(feed.id)
    # then
