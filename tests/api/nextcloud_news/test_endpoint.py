from fastapi.testclient import TestClient


def test_feed_creation(client: TestClient) -> None:
    # when
    response = client.post(
        "/feeds/",
        json={
            "url": "https://feedparser.readthedocs.io/en/latest/examples/atom10.xml",
            "folderId": None,
        },
    )
    # then
    assert response.status_code == 200
    # then
    feeds = response.json()["feeds"]
    assert len(feeds) == 1
    assert feeds[0]["url"] == "https://feedparser.readthedocs.io/en/latest/examples/atom10.xml"
    assert feeds[0]["title"] == "Sample Feed"
    assert feeds[0]["link"] == "http://example.org/"
    assert feeds[0]["updateErrorCount"] == 0
