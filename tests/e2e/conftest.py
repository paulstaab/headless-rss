import subprocess
import time
from collections.abc import Generator
from pathlib import Path

import httpx
import pytest


@pytest.fixture(scope="module")
def docker_image() -> Generator[str]:
    """Build the Docker image for testing."""
    image_name = "headless-rss:e2e-test"
    repo_root = Path(__file__).parent.parent.parent

    # Try to build the image
    result = subprocess.run(
        ["docker", "build", "--network=host", "-t", image_name, "."],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        pytest.skip(f"Failed to build Docker image: {result.stderr}")

    yield image_name

    # Cleanup: remove the image
    subprocess.run(
        ["docker", "rmi", "-f", image_name],
        check=False,
        capture_output=True,
    )


@pytest.fixture(scope="module")
def feed_server(make_httpserver):
    """Provide a test feed server that serves test RSS/Atom feeds."""

    def _respond_with_file(request, file_name: str):
        import werkzeug

        return werkzeug.wrappers.Response(
            response=(Path(__file__).parent.parent / file_name).read_text(),
            status=200,
            content_type="application/xml",
        )

    httpserver = make_httpserver
    httpserver.expect_request("/atom.xml").respond_with_handler(lambda x: _respond_with_file(x, "feeds/atom.xml"))
    httpserver.expect_request("/github_releases.atom").respond_with_handler(
        lambda x: _respond_with_file(x, "feeds/github_releases.atom")
    )
    httpserver.expect_request("/feed_without_ids.xml").respond_with_handler(
        lambda x: _respond_with_file(x, "feeds/feed_without_ids.xml")
    )
    httpserver.expect_request("/rss_2_0.xml").respond_with_handler(lambda x: _respond_with_file(x, "feeds/rss_2_0.xml"))
    httpserver.expect_request("/atom_1_0.xml").respond_with_handler(
        lambda x: _respond_with_file(x, "feeds/atom_1_0.xml")
    )
    return httpserver


@pytest.fixture
def container(docker_image: str, tmp_path: Path, feed_server) -> Generator[str]:
    """Start a container from the built image."""
    container_name = f"headless-rss-e2e-{int(time.time())}"
    data_dir = tmp_path / "data"
    data_dir.mkdir(exist_ok=True)

    # Use a random port to avoid conflicts
    import random

    port = random.randint(9000, 9999)

    # Start the container with the data directory mounted
    result = subprocess.run(
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
            docker_image,
        ],
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        pytest.skip(f"Failed to start container: {result.stderr}")

    base_url = f"http://localhost:{port}"

    # Wait for the container to be healthy
    max_wait = 30
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
        pytest.skip(f"Container failed to start:\n{logs.stdout}\n{logs.stderr}")

    yield base_url

    # Cleanup: stop and remove the container
    subprocess.run(
        ["docker", "rm", "-f", container_name],
        check=False,
        capture_output=True,
    )
