from typing import TYPE_CHECKING

from src.feed import now

if TYPE_CHECKING:
    from fastapi.testclient import TestClient


def test_get_items(client: TestClient, feed_server) -> None:
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
    response = client.get(
        "/items",
        params={
            "batchSize": 10,
            "offset": 0,
            "type": 0,
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
    timestamp = now()
    response = client.post(
        "/feeds",
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
            "lastModified": timestamp,
            "type": 0,
            "id": feed_id,
        },
    )

    # then
    assert response.status_code == 200
    items = response.json()["items"]
    assert len(items) == 1


def test_get_item_content(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.get(f"/items/{item_id}/content")

    # then
    assert response.status_code == 200
    payload = response.json()
    assert "content" in payload
    assert payload["content"]


def test_get_item_content_missing(client: TestClient) -> None:
    response = client.get("/items/999999/content")

    assert response.status_code == 404


def test_mark_item_as_read(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
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
    response = client.get("/items", params={"type": 0, "id": item_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is False
    assert items[0]["lastModified"] is not None  # P4fad


def test_mark_multiple_items_as_read(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.put(
        "/items/read/multiple",
        json={
            "items": [item_id],
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/items", params={"type": 3})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is False
    assert items[0]["lastModified"] is not None  # P4fad


def test_mark_item_as_unread(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.put(f"/items/{item_id}/unread")

    # then
    assert response.status_code == 200
    response = client.get("/items", params={"type": 0, "id": item_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is True
    assert items[0]["lastModified"] is not None  # P0bb5


def test_mark_multiple_items_as_unread(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.put(
        "/items/unread/multiple",
        json={
            "items": [item_id],
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/items", params={"type": 0, "id": item_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is True
    assert items[0]["lastModified"] is not None  # P0bb5


def test_mark_item_as_starred(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    feed_id = response.json()["feeds"][0]["id"]
    guid_hash = client.get("/items", params={"type": 3}).json()["items"][0]["guidHash"]

    # when
    response = client.put(f"/items/{feed_id}/{guid_hash}/star")

    # then
    assert response.status_code == 200
    response = client.get("/items", params={"type": 0, "id": feed_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["starred"] is True
    assert items[0]["lastModified"] is not None  # Pb10c


def test_mark_multiple_items_as_starred(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    feed_id = response.json()["feeds"][0]["id"]
    guid_hash = client.get("/items", params={"type": 3}).json()["items"][0]["guidHash"]

    # when
    response = client.put(
        "/items/star/multiple",
        json={
            "items": [{"guidHash": guid_hash, "feedId": feed_id}],
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/items", params={"type": 3})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["starred"] is True
    assert items[0]["lastModified"] is not None  # Pb10c


def test_mark_item_as_unstarred(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    feed_id = response.json()["feeds"][0]["id"]
    guid_hash = client.get("/items", params={"type": 3}).json()["items"][0]["guidHash"]

    # when
    response = client.put(f"/items/{feed_id}/{guid_hash}/unstar")

    # then
    assert response.status_code == 200
    response = client.get("/items", params={"type": 0, "id": feed_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["starred"] is False
    assert items[0]["lastModified"] is not None  # Pf6cb


def test_mark_multiple_items_as_unstarred(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    feed_id = response.json()["feeds"][0]["id"]
    guid_hash = client.get("/items", params={"type": 3}).json()["items"][0]["guidHash"]

    # when
    response = client.put(
        "/items/unstar/multiple",
        json={
            "items": [{"guidHash": guid_hash, "feedId": feed_id}],
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/items", params={"type": 3})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["starred"] is False
    assert items[0]["lastModified"] is not None  # Pf6cb


def test_mark_all_items_as_read(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    item_id = response.json()["newestItemId"]

    # when
    response = client.put(
        "/items/read",
        json={
            "newestItemId": item_id,
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/items", params={"type": 0, "id": item_id})
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is False
    assert items[0]["lastModified"] is not None  # P4fad
