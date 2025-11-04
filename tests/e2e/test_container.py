"""End-to-end test for headless-rss container.

This test validates a complete user journey against the Docker container image:
1. Adding a feed
2. Loading articles from the feed
3. Marking articles read and unread
4. Moving the feed to a different folder
5. Deleting the feed
"""

import httpx


def test_user_journey(container: str, feed_server) -> None:
    """Test a complete user journey through the RSS aggregator."""
    base_url = f"{container}/index.php/apps/news/api/v1-3"

    # Step 1: Verify the API is accessible
    response = httpx.get(f"{container}/status")
    assert response.status_code == 200
    assert response.json() == {"status": "ok"}

    # Step 2: Add a feed
    response = httpx.post(
        f"{base_url}/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    assert response.status_code == 200, f"Failed to add feed: {response.text}"
    feeds = response.json()["feeds"]
    assert len(feeds) == 1
    feed_id = feeds[0]["id"]
    assert feeds[0]["title"] == "Test Atom Feed"
    assert feeds[0]["url"] == feed_server.url_for("/atom.xml")

    # Step 3: Load articles from the feed
    response = httpx.get(
        f"{base_url}/items",
        params={
            "batchSize": 10,
            "offset": 0,
            "type": 0,  # type 0 = feed
            "id": feed_id,
            "getRead": True,
            "oldestFirst": False,
        },
    )
    assert response.status_code == 200, f"Failed to get items: {response.text}"
    items = response.json()["items"]
    assert len(items) == 1, f"Expected 1 item, got {len(items)}"
    item_id = items[0]["id"]
    assert items[0]["unread"] is True  # New items should be unread

    # Step 4: Mark article as read
    response = httpx.post(f"{base_url}/items/{item_id}/read")
    assert response.status_code == 200, f"Failed to mark item as read: {response.text}"

    # Verify the article is marked as read
    response = httpx.get(
        f"{base_url}/items",
        params={
            "type": 0,
            "id": feed_id,
            "getRead": True,
        },
    )
    assert response.status_code == 200
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is False, "Item should be marked as read"

    # Step 5: Mark article as unread
    response = httpx.post(f"{base_url}/items/{item_id}/unread")
    assert response.status_code == 200, f"Failed to mark item as unread: {response.text}"

    # Verify the article is marked as unread
    response = httpx.get(
        f"{base_url}/items",
        params={
            "type": 0,
            "id": feed_id,
            "getRead": True,
        },
    )
    assert response.status_code == 200
    items = response.json()["items"]
    assert len(items) == 1
    assert items[0]["unread"] is True, "Item should be marked as unread"

    # Step 6: Create a folder
    response = httpx.post(
        f"{base_url}/folders",
        json={
            "name": "Test Folder",
        },
    )
    assert response.status_code == 200, f"Failed to create folder: {response.text}"
    folders = response.json()["folders"]
    assert len(folders) == 1
    folder_id = folders[0]["id"]
    assert folders[0]["name"] == "Test Folder"

    # Step 7: Move the feed to the folder
    response = httpx.post(
        f"{base_url}/feeds/{feed_id}/move",
        json={
            "folderId": folder_id,
        },
    )
    assert response.status_code == 200, f"Failed to move feed: {response.text}"

    # Verify the feed is in the folder
    response = httpx.get(f"{base_url}/feeds")
    assert response.status_code == 200
    feeds = response.json()["feeds"]
    assert len(feeds) == 1
    assert feeds[0]["folderId"] == folder_id, "Feed should be in the new folder"

    # Step 8: Delete the feed
    response = httpx.delete(f"{base_url}/feeds/{feed_id}")
    assert response.status_code == 200, f"Failed to delete feed: {response.text}"

    # Verify the feed is deleted
    response = httpx.get(f"{base_url}/feeds")
    assert response.status_code == 200
    feeds = response.json()["feeds"]
    assert len(feeds) == 0, "Feed should be deleted"

    # Verify the folder still exists (deleting feed doesn't delete folder)
    response = httpx.get(f"{base_url}/folders")
    assert response.status_code == 200
    folders = response.json()["folders"]
    assert len(folders) == 1
    assert folders[0]["name"] == "Test Folder"
