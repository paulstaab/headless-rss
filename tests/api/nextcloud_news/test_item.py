from fastapi.testclient import TestClient


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


def test_get_updated_items(client: TestClient, feed_server) -> None:
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
        "/items/updated",
        params={
            "lastModified": 0,
            "type": 1,
            "id": feed_id,
        },
    )

    # then
    assert response.status_code == 200
    items = response.json()["items"]
    assert len(items) == 1


def test_mark_item_as_read(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds/",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.post(f"/items/{item_id}/read")

    # then
    assert response.status_code == 200
    response = client.get("/items/", params={"type": 1, "id": item_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is False


def test_mark_multiple_items_as_read(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds/",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.post(
        "/items/read/multiple",
        json={
            "itemIds": [item_id],
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/items/", params={"type": 1, "id": item_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is False


def test_mark_item_as_unread(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds/",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.post(f"/items/{item_id}/unread")

    # then
    assert response.status_code == 200
    response = client.get("/items/", params={"type": 1, "id": item_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is True


def test_mark_multiple_items_as_unread(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds/",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.post(
        "/items/unread/multiple",
        json={
            "itemIds": [item_id],
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/items/", params={"type": 1, "id": item_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is True


def test_mark_item_as_starred(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds/",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.post(f"/items/{item_id}/star")

    # then
    assert response.status_code == 200
    response = client.get("/items/", params={"type": 1, "id": item_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["starred"] is True


def test_mark_multiple_items_as_starred(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds/",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.post(
        "/items/star/multiple",
        json={
            "itemIds": [item_id],
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/items/", params={"type": 1, "id": item_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["starred"] is True


def test_mark_item_as_unstarred(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds/",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.post(f"/items/{item_id}/unstar")

    # then
    assert response.status_code == 200
    response = client.get("/items/", params={"type": 1, "id": item_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["starred"] is False


def test_mark_multiple_items_as_unstarred(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds/",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.post(
        "/items/unstar/multiple",
        json={
            "itemIds": [item_id],
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/items/", params={"type": 1, "id": item_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["starred"] is False


def test_mark_all_items_as_read(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds/",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.post(
        "/items/read",
        json={
            "newestItemId": item_id,
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/items/", params={"type": 1, "id": item_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is False
