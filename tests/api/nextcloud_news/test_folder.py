from fastapi.testclient import TestClient


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


def test_delete_folder(client: TestClient) -> None:
    # given
    response = client.post(
        "/folders/",
        json={
            "name": "Media",
        },
    )
    folder_id = response.json()["folders"][0]["id"]

    # when
    response = client.delete(f"/folders/{folder_id}")

    # then
    assert response.status_code == 200
    response = client.get("/folders/")
    folders = response.json()["folders"]
    assert len(folders) == 0


def test_delete_non_existent_folder(client: TestClient) -> None:
    # when
    response = client.delete("/folders/9999")

    # then
    assert response.status_code == 404
    assert response.json()["detail"] == "Folder not found"


def test_rename_folder(client: TestClient) -> None:
    # given
    response = client.post(
        "/folders/",
        json={
            "name": "Media",
        },
    )
    folder_id = response.json()["folders"][0]["id"]

    # when
    response = client.put(
        f"/folders/{folder_id}",
        json={
            "name": "News",
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/folders/")
    folders = response.json()["folders"]
    assert len(folders) == 1
    assert folders[0]["name"] == "News"


def test_rename_folder_already_exists(client: TestClient) -> None:
    # given
    client.post(
        "/folders/",
        json={
            "name": "Media",
        },
    )
    client.post(
        "/folders/",
        json={
            "name": "News",
        },
    )
    folder_id = client.get("/folders/").json()["folders"][0]["id"]

    # when
    response = client.put(
        f"/folders/{folder_id}",
        json={
            "name": "News",
        },
    )

    # then
    assert response.status_code == 409
    assert response.json()["detail"] == "Folder already exists"


def test_rename_folder_invalid_name(client: TestClient) -> None:
    # given
    response = client.post(
        "/folders/",
        json={
            "name": "Media",
        },
    )
    folder_id = response.json()["folders"][0]["id"]

    # when
    response = client.put(
        f"/folders/{folder_id}",
        json={
            "name": "",
        },
    )

    # then
    assert response.status_code == 422
    assert response.json()["detail"] == "Folder name is invalid"


def test_mark_items_read(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/folders/",
        json={
            "name": "Media",
        },
    )
    folder_id = response.json()["folders"][0]["id"]

    response = client.post(
        "/feeds/",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": folder_id,
        },
    )
    feed_id = response.json()["feeds"][0]["id"]

    # when
    response = client.post(
        f"/folders/{folder_id}/read",
        json={
            "newestItemId": feed_id,
        },
    )

    # then
    assert response.status_code == 200
    response = client.get("/items/")
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is False
