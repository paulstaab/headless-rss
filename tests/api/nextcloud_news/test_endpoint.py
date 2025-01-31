from fastapi.testclient import TestClient


def test_feed_creation(client: TestClient, feed_server) -> None:
    # when
    response = client.post(
        "/feeds/",
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
        "/feeds/",
        json={
            "url": f"{feed_server.url_for('/atom.xml')}",
            "folderId": None,
        },
    )
    # when
    response = client.post(
        "/feeds/",
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
        "/feeds/",
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
        "/feeds/",
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
    response = client.get("/feeds/")
    feeds = response.json()["feeds"]
    assert len(feeds) == 0


def test_delete_non_existent_feed(client: TestClient) -> None:
    # when
    response = client.delete("/feeds/9999")

    # then
    assert response.status_code == 404
    assert response.json()["detail"] == "Feed not found"


def test_get_items(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds/",
        json={
            "url": feed_server.url_for("/atom.xml"),
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


def test_get_folders(client: TestClient) -> None:
    # when
    response = client.get("/folders/")
    # then
    assert response.status_code == 200
    folders = response.json()["folders"]
    assert isinstance(folders, list)


def test_create_folder(client: TestClient) -> None:
    # when
    response = client.post(
        "/folders/",
        json={
            "name": "Media",
        },
    )
    # then
    assert response.status_code == 200
    folders = response.json()["folders"]
    assert len(folders) == 1
    assert folders[0]["name"] == "Media"


def test_create_folder_already_exists(client: TestClient) -> None:
    # given
    client.post(
        "/folders/",
        json={
            "name": "Media",
        },
    )
    # when
    response = client.post(
        "/folders/",
        json={
            "name": "Media",
        },
    )
    # then
    assert response.status_code == 409
    assert response.json()["detail"] == "Folder already exists"


def test_create_folder_invalid_name(client: TestClient) -> None:
    # when
    response = client.post(
        "/folders/",
        json={
            "name": "",
        },
    )
    # then
    assert response.status_code == 422
    assert response.json()["detail"] == "Folder name is invalid"
