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
    feeds = response.json()["feeds"]
    assert len(feeds) == 1
    assert feeds[0]["url"] == "https://feedparser.readthedocs.io/en/latest/examples/atom10.xml"
    assert feeds[0]["title"] == "Sample Feed"
    assert feeds[0]["link"] == "http://example.org/"
    assert feeds[0]["updateErrorCount"] == 0
    assert response.json()["newestItemId"] == feeds[0]["id"]


def test_feed_already_exists(client: TestClient) -> None:
    # given
    client.post(
        "/feeds/",
        json={
            "url": "https://feedparser.readthedocs.io/en/latest/examples/atom10.xml",
            "folderId": None,
        },
    )
    # when
    response = client.post(
        "/feeds/",
        json={
            "url": "https://feedparser.readthedocs.io/en/latest/examples/atom10.xml",
            "folderId": None,
        },
    )
    # then
    assert response.status_code == 409
    assert response.json()["detail"] == "Feed already exists"


def test_feed_cannot_be_read(client: TestClient) -> None:
    # when
    response = client.post(
        "/feeds/",
        json={
            "url": "https://invalid-url.com",
            "folderId": None,
        },
    )
    # then
    assert response.status_code == 422
    assert response.json()["detail"] == "Feed cannot be read"


def test_delete_feed(client: TestClient) -> None:
    # given
    response = client.post(
        "/feeds/",
        json={
            "url": "https://feedparser.readthedocs.io/en/latest/examples/atom10.xml",
            "folderId": None,
        },
    )
    feed_id = response.json()["feeds"][0]["id"]

    # when
    response = client.delete(f"/feeds/{feed_id}")

    # then
    assert response.status_code == 200
    response = client.get("/feeds/")
    feeds = response.json()["feeds"]
    assert len(feeds) == 0


def test_delete_non_existent_feed(client: TestClient) -> None:
    # when
    response = client.delete("/feeds/9999")

    # then
    assert response.status_code == 404
    assert response.json()["detail"] == "Feed not found"


def test_get_items(client: TestClient) -> None:
    # given
    response = client.post(
        "/feeds/",
        json={
            "url": "https://feedparser.readthedocs.io/en/latest/examples/atom10.xml",
            "folderId": None,
        },
    )
    feed_id = response.json()["feeds"][0]["id"]

    # when
    response = client.get(
        "/items/",
        params={
            "batchSize": 10,
            "offset": 0,
            "type": 1,
            "id": feed_id,
            "getRead": True,
            "oldestFirst": False,
        },
    )

    # then
    assert response.status_code == 200
    items = response.json()["items"]
    assert len(items) == 1
