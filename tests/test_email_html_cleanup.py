"""Tests for HTML cleanup functionality in email processing."""

from src.email import _clean_newsletter_html


class TestEmailHtmlCleanup:
    """Test HTML cleanup for newsletter content."""

    def test_removes_hidden_divs(self):
        """Test that hidden divs are removed."""
        html_input = """
        <div>Visible content</div>
        <div style="display: none; max-height: 0px; overflow: hidden;">Hidden content</div>
        <div>More visible content</div>
        """
        result = _clean_newsletter_html(html_input)
        assert "Hidden content" not in result
        assert "Visible content" in result
        assert "More visible content" in result

    def test_removes_layout_tables(self):
        """Test that empty layout tables are removed."""
        html_input = """
        <table align="center" border="0" cellpadding="0" cellspacing="0" class="container" width="600">
            <tbody><tr><td valign="top">
                <p>Actual content here</p>
            </td></tr></tbody>
        </table>
        """
        result = _clean_newsletter_html(html_input)
        assert "Actual content here" in result
        # Should not have the complex table structure
        assert 'align="center" border="0" cellpadding="0" cellspacing="0"' not in result

    def test_removes_meta_tags(self):
        """Test that meta tags are removed."""
        html_input = """
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width">
        <h1>Newsletter Title</h1>
        <p>Newsletter content</p>
        """
        result = _clean_newsletter_html(html_input)
        assert "<meta" not in result
        assert "Newsletter Title" in result
        assert "Newsletter content" in result

    def test_preserves_content_structure(self):
        """Test that actual content structure is preserved."""
        html_input = """
        <h1>AI Software Orchestration 🪄, Apache Kafka Origin 🤖, Terraform GUI 🪐</h1>
        <div>
            <h2>News & Trends</h2>
            <p>GitLab is evolving into an AI-native DevSecOps platform...</p>
            <h2>Resources & Tools</h2>
            <ul>
                <li>Parlant - AI framework</li>
                <li>Zod Codecs - TypeScript library</li>
            </ul>
        </div>
        """
        result = _clean_newsletter_html(html_input)
        assert "<h1>" in result and "</h1>" in result
        assert "<h2>" in result and "</h2>" in result
        assert "<p>" in result and "</p>" in result
        assert "<ul>" in result and "</ul>" in result
        assert "<li>" in result and "</li>" in result
        assert "GitLab is evolving" in result
        assert "Parlant" in result

    def test_complex_newsletter_cleanup(self):
        """Test cleanup of complex newsletter HTML structure."""
        # Simplified version of the actual problematic HTML
        html_input = """
        <h1>TLDR DevOps</h1>
        <meta charset="UTF-8">
        <div style="display: none; max-height: 0px; overflow: hidden;">Hidden preview text</div>
        <table align="center" class="document">
            <tbody><tr><td valign="top">
                <table align="center" border="0" cellpadding="0" cellspacing="0" class="container" width="600">
                    <tbody><tr class="inner-body"><td>
                        <h2>News & Trends</h2>
                        <p>GitLab 18.3: Expanding AI orchestration in software engineering</p>
                        <p>AWS Lambda now supports GitHub Actions to simplify function deployment</p>
                    </td></tr></tbody>
                </table>
            </td></tr></tbody>
        </table>
        """
        result = _clean_newsletter_html(html_input)

        # Should preserve actual content
        assert "TLDR DevOps" in result
        assert "News & Trends" in result
        assert "GitLab 18.3" in result
        assert "AWS Lambda" in result

        # Should remove problematic elements
        assert "display: none" not in result
        assert "Hidden preview text" not in result
        assert "<meta" not in result
        assert 'border="0" cellpadding="0" cellspacing="0"' not in result

    def test_handles_empty_input(self):
        """Test that empty input is handled gracefully."""
        result = _clean_newsletter_html("")
        assert result == ""

    def test_handles_none_input(self):
        """Test that None input is handled gracefully."""
        result = _clean_newsletter_html(None)
        assert result == ""

    def test_removes_tracking_pixels(self):
        """Test that tracking pixels and images are removed."""
        html_input = """
        <h1>Newsletter</h1>
        <img src="https://tracking.example.com/pixel.gif" width="1" height="1" style="display:none">
        <img src="https://images.example.com/logo.png" alt="Logo" width="200">
        <p>Content here</p>
        """
        result = _clean_newsletter_html(html_input)
        assert "Newsletter" in result
        assert "Content here" in result
        # Should remove tracking pixels but may keep regular images
        assert "tracking.example.com" not in result
