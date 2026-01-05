"""Tests for media thumbnail extraction functionality."""

from src import article, feed, folder


class TestExtractFirstImageUrl:
    """Tests for the extract_first_image_url function."""

    def test_extracts_image_with_double_quotes(self):
        """Test extracting image URL with double quotes."""
        html = '<p>Some text</p><img src="https://example.com/image.jpg" alt="test"><p>More text</p>'
        result = article.extract_first_image_url(html)
        assert result == "https://example.com/image.jpg"

    def test_extracts_image_with_single_quotes(self):
        """Test extracting image URL with single quotes."""
        html = "<p>Some text</p><img src='https://example.com/image.jpg' alt='test'><p>More text</p>"
        result = article.extract_first_image_url(html)
        assert result == "https://example.com/image.jpg"

    def test_extracts_first_image_when_multiple_exist(self):
        """Test that only the first image is extracted when multiple exist."""
        html = """
        <p>Text</p>
        <img src="https://example.com/first.jpg" alt="first">
        <p>More text</p>
        <img src="https://example.com/second.jpg" alt="second">
        """
        result = article.extract_first_image_url(html)
        assert result == "https://example.com/first.jpg"

    def test_handles_image_with_additional_attributes(self):
        """Test extracting image with various attributes."""
        html = '<img class="thumbnail" width="100" src="https://example.com/image.jpg" height="100" alt="test">'
        result = article.extract_first_image_url(html)
        assert result == "https://example.com/image.jpg"

    def test_returns_none_when_no_image(self):
        """Test that None is returned when no image exists."""
        html = "<p>Just some text without images</p>"
        result = article.extract_first_image_url(html)
        assert result is None

    def test_returns_none_for_empty_content(self):
        """Test that None is returned for empty content."""
        result = article.extract_first_image_url("")
        assert result is None

    def test_returns_none_for_none_content(self):
        """Test that None is returned for None content."""
        result = article.extract_first_image_url(None)
        assert result is None

    def test_handles_relative_urls(self):
        """Test extracting relative URLs."""
        html = '<img src="/images/photo.jpg" alt="photo">'
        result = article.extract_first_image_url(html)
        assert result == "/images/photo.jpg"

    def test_handles_data_urls(self):
        """Test extracting data URLs."""
        html = '<img src="data:image/png;base64,iVBORw0KGgoAAAANS" alt="inline">'
        result = article.extract_first_image_url(html)
        assert result == "data:image/png;base64,iVBORw0KGgoAAAANS"

    def test_case_insensitive_img_tag(self):
        """Test that IMG tags in different cases are recognized."""
        html = '<IMG SRC="https://example.com/image.jpg" ALT="test">'
        result = article.extract_first_image_url(html)
        assert result == "https://example.com/image.jpg"


class TestArticleCreationWithMediaThumbnail:
    """Tests for article creation with media thumbnail."""

    def test_uses_provided_media_thumbnail(self):
        """Test that provided media_thumbnail is used."""
        content = '<p>Text with <img src="https://example.com/content-image.jpg"></p>'
        new_article = article.create(
            feed_id=1,
            title="Test Article",
            author="Test Author",
            url="https://example.com/article",
            content=content,
            guid="test-guid-1",
            media_thumbnail="https://example.com/explicit-thumbnail.jpg",
        )
        assert new_article.media_thumbnail == "https://example.com/explicit-thumbnail.jpg"

    def test_falls_back_to_first_image_when_no_thumbnail(self):
        """Test that first image from content is used when no thumbnail provided."""
        content = '<p>Text with <img src="https://example.com/content-image.jpg"></p>'
        new_article = article.create(
            feed_id=1,
            title="Test Article",
            author="Test Author",
            url="https://example.com/article",
            content=content,
            guid="test-guid-2",
        )
        assert new_article.media_thumbnail == "https://example.com/content-image.jpg"

    def test_media_thumbnail_is_none_when_no_images(self):
        """Test that media_thumbnail is None when no images exist."""
        content = "<p>Just text without any images</p>"
        new_article = article.create(
            feed_id=1,
            title="Test Article",
            author="Test Author",
            url="https://example.com/article",
            content=content,
            guid="test-guid-3",
        )
        assert new_article.media_thumbnail is None

    def test_media_thumbnail_is_none_when_content_is_none(self):
        """Test that media_thumbnail is None when content is None."""
        new_article = article.create(
            feed_id=1,
            title="Test Article",
            author="Test Author",
            url="https://example.com/article",
            content=None,
            guid="test-guid-4",
        )
        assert new_article.media_thumbnail is None


class TestFeedIntegrationWithMediaThumbnail:
    """Integration tests for media thumbnail extraction from feeds."""

    def test_feed_parsing_extracts_thumbnails_from_content(self, feed_server):
        """Test that thumbnails are extracted from feed content."""
        # Setup
        root_folder_id = folder.get_root_folder_id()
        feed_url = feed_server.url_for("/feed_with_images.xml")

        # Add feed and parse articles
        new_feed = feed.add(feed_url, root_folder_id)
        articles = article.get_by_feed(new_feed.id)

        # Verify we got all articles
        assert len(articles) == 3

        # Article 1: Single image in content
        article1 = [a for a in articles if "Article with Image in Content" in (a.title or "")][0]
        assert article1.media_thumbnail == "https://example.com/image1.jpg"

        # Article 2: Multiple images, should use first one
        article2 = [a for a in articles if "Article with Multiple Images" in (a.title or "")][0]
        assert article2.media_thumbnail == "https://example.com/first.jpg"

        # Article 3: No images
        article3 = [a for a in articles if "Article without Images" in (a.title or "")][0]
        assert article3.media_thumbnail is None
