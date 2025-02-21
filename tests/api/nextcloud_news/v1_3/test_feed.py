from fastapi.testclient import TestClient


def test_feed_creation(client: TestClient, feed_server) -> None:
    # when
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    # then
    assert response.status_code == 200
    feeds = response.json()["feeds"]
    assert len(feeds) == 1
    assert feeds[0]["url"] == feed_server.url_for("/atom.xml")
    assert feeds[0]["title"] == "Test Atom Feed"
    assert feeds[0]["link"] == "http://example.org/"
    assert feeds[0]["updateErrorCount"] == 0
    assert response.json()["newestItemId"] == feeds[0]["id"]


def test_feed_already_exists(client: TestClient, feed_server) -> None:
    # given
    client.post(
        "/feeds",
        json={
            "url": f"{feed_server.url_for('/atom.xml')}",
            "folderId": None,
        },
    )
    # when
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    # then
    assert response.status_code == 409
    assert response.json()["detail"] == "Feed already exists"


def test_feed_cannot_be_read(client: TestClient, feed_server) -> None:
    # when
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/non-existant.xml"),
            "folderId": None,
        },
    )
    # then
    assert response.status_code == 422
    assert response.json()["detail"] == "Feed cannot be read"


def test_delete_feed(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    feed_id = response.json()["feeds"][0]["id"]

    # when
    response = client.delete(f"/feeds/{feed_id}")

    # then
    assert response.status_code == 200
    response = client.get("/feeds")
    feeds = response.json()["feeds"]
    assert len(feeds) == 0


def test_delete_non_existent_feed(client: TestClient) -> None:
    # when
    response = client.delete("/feeds/9999")

    # then
    assert response.status_code == 404
    assert response.json()["detail"] == "Feed not found"


def test_move_feed(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    feed_id = response.json()["feeds"][0]["id"]

    response = client.post(
        "/folders",
        json={
            "name": "New Folder",
        },
    )
    folder_id = response.json()["folders"][0]["id"]

    # when
    response = client.post(
        f"/feeds/{feed_id}/move",
        json={
            "folderId": folder_id,
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/feeds")
    feeds = response.json()["feeds"]
    assert len(feeds) == 1
    assert feeds[0]["folderId"] == folder_id


def test_rename_feed(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    feed_id = response.json()["feeds"][0]["id"]

    # when
    response = client.post(
        f"/feeds/{feed_id}/rename",
        json={
            "feedTitle": "New Title",
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/feeds")
    feeds = response.json()["feeds"]
    assert len(feeds) == 1
    assert feeds[0]["title"] == "New Title"


def test_mark_items_read(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    feed_id = response.json()["feeds"][0]["id"]

    # when
    response = client.post(
        f"/feeds/{feed_id}/read",
        json={
            "newestItemId": 10,
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/items", params={"type": 0, "id": feed_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is False


def test_feed_creation_with_folder_id_zero(client: TestClient, feed_server) -> None:
    # when
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": 0,
        },
    )
    # then
    assert response.status_code == 200
    feeds = response.json()["feeds"]
    assert len(feeds) == 1
    assert feeds[0]["url"] == feed_server.url_for("/atom.xml")
    assert feeds[0]["title"] == "Test Atom Feed"
    assert feeds[0]["link"] == "http://example.org/"
    assert feeds[0]["updateErrorCount"] == 0
    assert feeds[0]["folderId"] is None
    assert response.json()["newestItemId"] == feeds[0]["id"]


def test_next_update_time_calculation(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    feed_id = response.json()["feeds"][0]["id"]

    # when
    response = client.get("/feeds")
    feeds = response.json()["feeds"]

    # then
    assert response.status_code == 200
    assert len(feeds) == 1
    assert feeds[0]["nextUpdateTime"] is not None
