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
