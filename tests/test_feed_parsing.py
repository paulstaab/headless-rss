import pytest
from src import database, folder
from src.feed import _create, update


@pytest.fixture(params=["/atom.xml", "/github_releases.atom"])
def feed_url(request, feed_server):
    return feed_server.url_for(request.param)


def test_feed_parsing(feed_url):
    # given
    root_folder_id = folder.get_root_folder_id()
    # when
    with database.get_session() as db:
        feed = _create(feed_url, folder_id=root_folder_id)
        db.add(feed)
        db.commit()
        db.refresh(feed)
        update(feed.id)
    # then
    with database.get_session() as db:
        articles = db.query(database.Article).filter(database.Article.feed_id == feed.id).all()
    assert articles
