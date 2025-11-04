#!/usr/bin/env python3
"""Standalone E2E test script for testing the Docker container image.

This script is designed to run in CI after the Docker image is built,
testing the actual image that will be pushed to the registry.

Usage:
    python tests/e2e/run_e2e_test.py <image_name>

Example:
    python tests/e2e/run_e2e_test.py ghcr.io/paulstaab/headless-rss:latest
"""

import random
import subprocess
import sys
import time
from pathlib import Path

import httpx


def start_feed_server() -> tuple[subprocess.Popen, int]:
    """Start a simple HTTP server for serving test feeds."""
    # Find an available port
    port = random.randint(40000, 49999)
    feeds_dir = Path(__file__).parent.parent / "feeds"

    # Start Python's built-in HTTP server
    process = subprocess.Popen(
        [sys.executable, "-m", "http.server", str(port)],
        cwd=feeds_dir,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    # Wait for server to start
    time.sleep(2)

    return process, port


def run_container(image_name: str, data_dir: Path) -> tuple[str, int]:
    """Start a container from the specified image."""
    container_name = f"headless-rss-e2e-{int(time.time())}"

    # Use a random port to avoid conflicts
    port = random.randint(9000, 9999)

    # Start the container
    subprocess.run(
        [
            "docker",
            "run",
            "-d",
            "--name",
            container_name,
            "-p",
            f"{port}:8000",
            "-v",
            f"{data_dir}:/app/data",
            "--network",
            "host",  # Use host network to access feed_server
            image_name,
        ],
        capture_output=True,
        text=True,
        check=True,
    )

    # Wait for the container to be healthy
    max_wait = 30
    base_url = f"http://localhost:{port}"
    for _i in range(max_wait):
        try:
            response = httpx.get(f"{base_url}/status", timeout=1)
            if response.status_code == 200:
                break
        except (httpx.ConnectError, httpx.TimeoutException):
            pass
        time.sleep(1)
    else:
        # If we couldn't connect, get the logs and fail
        logs = subprocess.run(
            ["docker", "logs", container_name],
            capture_output=True,
            text=True,
        )
        subprocess.run(["docker", "rm", "-f", container_name], check=False)
        raise RuntimeError(f"Container failed to start:\n{logs.stdout}\n{logs.stderr}")

    return container_name, port


def test_user_journey(base_url: str, feed_server_url: str) -> None:
    """Test a complete user journey through the RSS aggregator."""
    api_base = f"{base_url}/index.php/apps/news/api/v1-3"
    feed_url = f"{feed_server_url}/atom.xml"

    print("Step 1: Verify the API is accessible")
    response = httpx.get(f"{base_url}/status")
    assert response.status_code == 200, f"Status check failed: {response.text}"
    assert response.json() == {"status": "ok"}
    print("  ✓ API is accessible")

    print("Step 2: Add a feed")
    response = httpx.post(
        f"{api_base}/feeds",
        json={"url": feed_url, "folderId": None},
    )
    assert response.status_code == 200, f"Failed to add feed: {response.text}"
    feeds = response.json()["feeds"]
    assert len(feeds) == 1
    feed_id = feeds[0]["id"]
    assert feeds[0]["title"] == "Test Atom Feed"
    print(f"  ✓ Feed added with ID {feed_id}")

    print("Step 3: Load articles from the feed")
    response = httpx.get(
        f"{api_base}/items",
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
    assert items[0]["unread"] is True
    print(f"  ✓ Loaded {len(items)} article(s)")

    print("Step 4: Mark article as read")
    response = httpx.post(f"{api_base}/items/{item_id}/read")
    assert response.status_code == 200, f"Failed to mark item as read: {response.text}"
    response = httpx.get(f"{api_base}/items", params={"type": 0, "id": feed_id, "getRead": True})
    assert response.status_code == 200
    items = response.json()["items"]
    assert items[0]["unread"] is False, "Item should be marked as read"
    print("  ✓ Article marked as read")

    print("Step 5: Mark article as unread")
    response = httpx.post(f"{api_base}/items/{item_id}/unread")
    assert response.status_code == 200, f"Failed to mark item as unread: {response.text}"
    response = httpx.get(f"{api_base}/items", params={"type": 0, "id": feed_id, "getRead": True})
    assert response.status_code == 200
    items = response.json()["items"]
    assert items[0]["unread"] is True, "Item should be marked as unread"
    print("  ✓ Article marked as unread")

    print("Step 6: Create a folder")
    response = httpx.post(f"{api_base}/folders", json={"name": "Test Folder"})
    assert response.status_code == 200, f"Failed to create folder: {response.text}"
    folders = response.json()["folders"]
    assert len(folders) == 1
    folder_id = folders[0]["id"]
    assert folders[0]["name"] == "Test Folder"
    print(f"  ✓ Folder created with ID {folder_id}")

    print("Step 7: Move the feed to the folder")
    response = httpx.post(f"{api_base}/feeds/{feed_id}/move", json={"folderId": folder_id})
    assert response.status_code == 200, f"Failed to move feed: {response.text}"
    response = httpx.get(f"{api_base}/feeds")
    assert response.status_code == 200
    feeds = response.json()["feeds"]
    assert len(feeds) == 1
    assert feeds[0]["folderId"] == folder_id, "Feed should be in the new folder"
    print("  ✓ Feed moved to folder")

    print("Step 8: Delete the feed")
    response = httpx.delete(f"{api_base}/feeds/{feed_id}")
    assert response.status_code == 200, f"Failed to delete feed: {response.text}"
    response = httpx.get(f"{api_base}/feeds")
    assert response.status_code == 200
    feeds = response.json()["feeds"]
    assert len(feeds) == 0, "Feed should be deleted"
    print("  ✓ Feed deleted")

    print("Step 9: Verify folder still exists")
    response = httpx.get(f"{api_base}/folders")
    assert response.status_code == 200
    folders = response.json()["folders"]
    assert len(folders) == 1
    assert folders[0]["name"] == "Test Folder"
    print("  ✓ Folder still exists after feed deletion")


def main() -> int:
    """Main entry point for the E2E test script."""
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <image_name>")
        print(f"Example: {sys.argv[0]} ghcr.io/paulstaab/headless-rss:latest")
        return 1

    image_name = sys.argv[1]
    print(f"Testing Docker image: {image_name}")
    print("=" * 60)

    # Create temporary data directory
    import tempfile

    data_dir = Path(tempfile.mkdtemp())

    feed_server = None
    container_name = None

    try:
        # Start feed server
        print("\nStarting feed server...")
        feed_server, feed_port = start_feed_server()
        feed_server_url = f"http://localhost:{feed_port}"
        print(f"  ✓ Feed server started on port {feed_port}")

        # Start container
        print(f"\nStarting container from image {image_name}...")
        container_name, container_port = run_container(image_name, data_dir)
        base_url = f"http://localhost:{container_port}"
        print(f"  ✓ Container started on port {container_port}")

        # Run the test
        print("\nRunning E2E test...")
        print("-" * 60)
        test_user_journey(base_url, feed_server_url)
        print("-" * 60)
        print("\n✅ All E2E tests passed!")

        return 0

    except Exception as e:
        print(f"\n❌ E2E test failed: {e}")
        import traceback

        traceback.print_exc()
        return 1

    finally:
        # Cleanup
        print("\nCleaning up...")
        if container_name:
            subprocess.run(
                ["docker", "rm", "-f", container_name],
                check=False,
                capture_output=True,
            )
            print(f"  ✓ Container {container_name} removed")

        if feed_server:
            feed_server.terminate()
            feed_server.wait(timeout=5)
            print("  ✓ Feed server stopped")

        # Clean up temp directory
        import shutil

        if data_dir.exists():
            shutil.rmtree(data_dir)
            print("  ✓ Temporary data directory removed")


if __name__ == "__main__":
    sys.exit(main())
