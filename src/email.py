import imaplib
import json
import logging
import re
import time
from email.header import decode_header
from email.parser import BytesParser
from html.parser import HTMLParser

from openai import OpenAI

from src import article, database, feed, folder
from src.database import EmailCredential, get_session
from src.options import Options

logger = logging.getLogger(__name__)


OPENAI_TIMEOUT_SECONDS = 10
NEWSLETTER_MAX_CHARS = 5000
NEWSLETTER_MAX_ITEMS = 25


class NewsletterHTMLCleaner(HTMLParser):
    """HTML parser to clean up newsletter content for better readability."""

    def __init__(self):
        super().__init__()
        self.result = []
        self.skip_content = False
        self.in_hidden_div = False
        self.table_depth = 0
        self.in_layout_table = False

    def handle_starttag(self, tag, attrs):  # noqa: C901
        """Handle start tags, filtering out problematic elements."""
        # Skip meta tags completely
        if tag == "meta":
            return

        # Check for hidden divs
        if self._is_hidden_div(tag, attrs):
            return

        # Handle tables
        if self._handle_table_start(tag, attrs):
            return

        # Skip table-related tags when in layout table mode
        if self.in_layout_table and tag in ["tbody", "tr", "td", "th"]:
            return

        # For other tags, keep them but clean up attributes
        self._output_cleaned_tag(tag, attrs)

    def _is_hidden_div(self, tag, attrs):
        """Check if this is a hidden div that should be skipped."""
        if tag == "div":
            attrs_dict = dict(attrs)
            style = attrs_dict.get("style", "")
            if "display: none" in style or "display:none" in style:
                self.in_hidden_div = True
                return True
        return False

    def _handle_table_start(self, tag, attrs):
        """Handle table start tags, converting layout tables to divs."""
        if tag == "table":
            self.table_depth += 1
            # Check if this looks like a layout table (common newsletter pattern)
            attrs_dict = dict(attrs)
            if (
                attrs_dict.get("border") == "0"
                and attrs_dict.get("cellpadding") == "0"
                and attrs_dict.get("cellspacing") == "0"
            ):
                # This is likely a layout table, convert to div
                self.in_layout_table = True
                self.result.append("<div>")
                return True
            else:
                # Keep regular tables but clean attributes
                self.result.append("<table>")
                return True
        return False

    def _output_cleaned_tag(self, tag, attrs):
        """Output a tag with cleaned attributes."""
        clean_attrs = []
        for name, value in attrs:
            # Keep essential attributes, remove styling and layout attributes
            if name in ["href", "src", "alt", "title", "id"]:
                # Filter out tracking URLs
                if name == "src" and ("tracking" in value or "pixel" in value):
                    continue
                clean_attrs.append(f'{name}="{value}"')

        if clean_attrs:
            self.result.append(f"<{tag} {' '.join(clean_attrs)}>")
        else:
            self.result.append(f"<{tag}>")

    def handle_endtag(self, tag):
        """Handle end tags."""
        # Skip meta tags
        if tag == "meta":
            return

        # Handle hidden div end
        if tag == "div" and self.in_hidden_div:
            self.in_hidden_div = False
            return

        # Handle table structure
        if tag == "table":
            self.table_depth -= 1
            if self.in_layout_table:
                self.in_layout_table = False
                self.result.append("</div>")
            else:
                self.result.append("</table>")
            return

        # Skip table-related tags when in layout table mode
        if self.in_layout_table and tag in ["tbody", "tr", "td", "th"]:
            return

        self.result.append(f"</{tag}>")

    def handle_data(self, data):
        """Handle text data, skipping content in hidden elements."""
        if not self.in_hidden_div:
            # Clean up excessive whitespace
            cleaned_data = re.sub(r"\s+", " ", data.strip())
            if cleaned_data:
                self.result.append(cleaned_data)

    def get_cleaned_html(self):
        """Get the cleaned HTML result."""
        return "".join(self.result)


def _clean_newsletter_html(html_content):
    """Clean HTML content from newsletters to improve readability."""
    if not html_content:
        return ""

    # Remove tracking pixels and small images first
    html_content = re.sub(
        r'<img[^>]*(?:width="1"|height="1"|style="[^"]*display\s*:\s*none)[^>]*>', "", html_content, flags=re.IGNORECASE
    )

    # Parse and clean the HTML
    cleaner = NewsletterHTMLCleaner()
    try:
        cleaner.feed(html_content)
        cleaned = cleaner.get_cleaned_html()

        # Post-process: remove empty divs and excessive whitespace
        cleaned = re.sub(r"<div>\s*</div>", "", cleaned)
        cleaned = re.sub(r"\s+", " ", cleaned)
        cleaned = cleaned.strip()

        return cleaned
    except Exception as e:
        logger.warning(f"Failed to clean HTML content: {e}")
        # Fallback: basic cleanup with regex
        html_content = re.sub(r"<meta[^>]*>", "", html_content, flags=re.IGNORECASE)
        html_content = re.sub(
            r'<div[^>]*style="[^"]*display\s*:\s*none[^"]*"[^>]*>.*?</div>',
            "",
            html_content,
            flags=re.IGNORECASE | re.DOTALL,
        )
        return html_content


class EmailConnectionError(Exception):
    """Raised when there is an error connecting to the email server."""


def add_credentials(protocol, server, port, username, password):
    """Store email credentials in the database."""
    with get_session() as session:
        credential = EmailCredential(protocol=protocol, server=server, port=port, username=username, password=password)
        # test that the credentials work
        try:
            _ = _connect_to_mailbox(credential)
        except imaplib.IMAP4.error as e:
            raise EmailConnectionError(
                f"Failed to connect to mailbox at {server}:{port} for user {username}: {e}"
            ) from e
        session.add(credential)
        session.commit()


def fetch_emails_from_all_mailboxes() -> None:
    with get_session() as session:
        credentials = session.query(EmailCredential).all()
    if not credentials:
        logger.info("No email credentials configured. Skipping email fetch.")
        clean_up_old_newsletters()
        return
    try:
        for credential in credentials:
            fetch_emails(credential)
    finally:
        clean_up_old_newsletters()


def fetch_emails(credential: database.EmailCredential) -> None:  # noqa: C901
    """Fetch emails from all configured mailboxes."""
    if str(credential.protocol) != "imap":
        raise NotImplementedError(f"Protocol '{credential.protocol}' not supported. Only 'imap' is implemented.")

    logger.info(f"Fetching emails for {credential.username} from {credential.server}:{credential.port}")

    mail = _connect_to_mailbox(credential)
    logger.debug("IMAP: Logged in and selected inbox.")
    status, messages = mail.search(None, "UNSEEN")
    if status != "OK":
        logger.error(f"IMAP: Failed search for {credential.username}. Status: {status}")
        return
    email_ids = messages[0].split()
    logger.info(f"IMAP: Found {len(email_ids)} emails for {credential.username}.")
    for num in email_ids:
        fetch_status, data = mail.fetch(num, "(RFC822)")
        if fetch_status == "OK" and data and data[0] is not None:
            raw_email = data[0][1]
            process_email(raw_email)
            mail.store(num, "+FLAGS", "\\Seen")
        else:
            logger.warning(
                f"IMAP: Failed fetch email ID {num.decode()} for {credential.username}. Status: {fetch_status}"
            )
    mail.logout()
    logger.debug(f"IMAP: Logged out for {credential.username}.")


def _connect_to_mailbox(credential):
    mail = imaplib.IMAP4_SSL(credential.server, credential.port)  # type: ignore
    mail.login(credential.username, credential.password)  # type: ignore
    mail.select("inbox")
    return mail


def process_email(raw_email) -> None:  # noqa: C901
    """Process a raw email message."""
    msg = BytesParser().parsebytes(raw_email)
    subject = _extract_email_subject(msg)
    from_address = _extract_sender_address(msg)
    logger.debug(f"Processing email: Subject='{subject}', From='{from_address}'")

    if not _is_mailing_list(msg):
        logger.info(f"Ignoring non-mailing list email: Subject='{subject}', From='{from_address}'")
        return

    feed_title = _extract_feed_title(msg)
    logger.info(f"Identified mailing list email: Subject='{subject}', List='{feed_title}'")

    with get_session() as session:
        # Check if a feed exists for this mailing list
        existing_feed = session.query(database.Feed).filter(database.Feed.url == from_address).first()

        if not existing_feed:
            logger.info(f"No existing feed found for '{from_address}'. Creating new feed.")
            # Assuming folder_id=1 is a default/root folder. Adjust as needed.
            # We might need a better way to determine the folder.

            new_feed = feed.add_mailing_list(
                from_address=from_address, title=feed_title, folder_id=folder.get_root_folder_id()
            )
            logger.info(f"Created new feed '{feed_title}' with ID {new_feed.id}")
            feed_id = new_feed.id
        else:
            logger.debug(f"Found existing feed '{from_address}' with ID {existing_feed.id}")
            feed_id = existing_feed.id

        # Extract content (this might need refinement based on email structure)
        content = ""
        if msg.is_multipart():
            for part in msg.walk():
                content_type = part.get_content_type()
                content_disposition = str(part.get("Content-Disposition"))
                if content_type == "text/html" and "attachment" not in content_disposition:
                    content = part.get_payload(decode=True)  # type: ignore
                    break  # Prefer html text
            if not content:  # Fallback to plain
                for part in msg.walk():
                    content_type = part.get_content_type()
                    content_disposition = str(part.get("Content-Disposition"))
                    if content_type == "text/plain" and "attachment" not in content_disposition:
                        content = part.get_payload(decode=True)  # type: ignore
                        break
        else:
            content = msg.get_payload(decode=True)  # type: ignore

        # Ensure content is a string
        if isinstance(content, bytes):
            # Attempt decoding with message charset or fallback to utf-8
            charset = msg.get_content_charset() or "utf-8"
            try:
                content = content.decode(charset, errors="replace")
            except LookupError, UnicodeDecodeError:
                content = content.decode("utf-8", errors="replace")  # Fallback
        elif not isinstance(content, str):
            content = str(content)  # Convert other types to string

        # Clean HTML content for better readability in RSS readers
        if content and content.strip().startswith("<"):
            content = _clean_newsletter_html(content)

        new_articles = _create_articles_from_email(
            feed_id=feed_id,
            subject=subject,
            from_address=from_address,
            content=content,
        )
        added_count = 0
        for new_article in new_articles:
            existing_article = session.query(database.Article).filter_by(guid_hash=new_article.guid_hash).first()
            if existing_article:
                continue
            session.add(new_article)
            added_count += 1
        if added_count:
            session.commit()
            logger.info(f"Added {added_count} email article(s) for '{subject}' to feed '{feed_title}'")


def _extract_sender_address(msg) -> str:
    from_address = msg["from"].split("<")[-1].strip(">").strip() if "<" in msg["from"] else msg["from"]
    return from_address


def _extract_feed_title(msg) -> str:
    if "<" in msg["from"] and ">" in msg["from"]:
        feed_title = msg["from"].split("<")[0].strip()
    else:
        feed_title = msg["from"].split("@")[1].split(".")[0]
    return feed_title


def _extract_email_subject(msg) -> str:
    raw_subject = msg["subject"]
    decoded_subject_parts = decode_header(raw_subject)
    subject = ""
    for part, encoding in decoded_subject_parts:
        if isinstance(part, bytes):
            subject += part.decode(encoding or "utf-8", errors="replace")
        else:
            subject += part
    return subject


def _is_mailing_list(msg) -> bool:
    """Check if the email is from a mailing list."""
    return "List-Unsubscribe" in msg


def _create_article_from_email(
    feed_id: int,
    subject: str,
    from_address: str,
    content: str,
    summary: str | None = None,
    url: str | None = None,
) -> database.Article:
    """Create an article from email data."""
    guid = f"{from_address}:{subject}" if not url else f"{from_address}:{subject}:{url}"

    new_article = article.create(
        feed_id=feed_id,
        title=subject,
        author=from_address,
        content=content,
        guid=guid,
        url=url,
        media_description=summary,
    )

    return new_article


def _create_articles_from_email(
    feed_id: int,
    subject: str,
    from_address: str,
    content: str,
) -> list[database.Article]:
    llm_result = _parse_newsletter_with_llm(subject=subject, from_address=from_address, content=content)
    if not llm_result:
        return [_create_article_from_email(feed_id, subject, from_address, content)]

    if llm_result.get("mode") == "multi":
        items = llm_result.get("items") or []
        articles: list[database.Article] = []
        for item in items[:NEWSLETTER_MAX_ITEMS]:
            url = item.get("url")
            if not url:
                continue
            title = item.get("title") or subject
            item_summary = item.get("summary")
            item_content = item.get("content") or item_summary or ""
            articles.append(
                _create_article_from_email(
                    feed_id=feed_id,
                    subject=title,
                    from_address=from_address,
                    content=item_content,
                    summary=item_summary,
                    url=url,
                )
            )
        if articles:
            return articles

    summary = llm_result.get("summary")
    formatted_content = llm_result.get("content") or content
    return [_create_article_from_email(feed_id, subject, from_address, formatted_content, summary=summary)]


def _parse_newsletter_with_llm(subject: str, from_address: str, content: str) -> dict | None:
    if not _llm_enabled():
        return None

    trimmed_content = _trim_newsletter_content(content)
    if not trimmed_content:
        return None

    try:
        response_obj = _call_openai_responses_api(subject, from_address, trimmed_content)
    except Exception as exc:
        logger.warning(f"LLM newsletter parsing failed: {exc}")
        return None

    response_text = _extract_openai_response_text(response_obj)
    if not response_text:
        return None

    try:
        parsed = json.loads(response_text)
    except json.JSONDecodeError:
        logger.warning("LLM response was not valid JSON")
        return None

    return _normalize_llm_result(parsed)


def _llm_enabled() -> bool:
    return Options.get().llm_enabled


def _trim_newsletter_content(content: str) -> str:
    if not content:
        return ""
    return content[:NEWSLETTER_MAX_CHARS]


def _call_openai_responses_api(subject: str, from_address: str, content: str):
    api_key = Options.get().openai_api_key
    if not api_key:
        raise ValueError("OPENAI_API_KEY is not set")

    payload = _build_openai_payload(subject, from_address, content)
    client = OpenAI(api_key=api_key, timeout=OPENAI_TIMEOUT_SECONDS)
    return client.chat.completions.create(**payload)


def _build_openai_payload(subject: str, from_address: str, content: str) -> dict:
    options = Options.get()
    instructions = (
        "You are parsing a newsletter email. Decide if it is a list of distinct article links "
        "with short descriptions. If yes, return mode=multi with an ordered list of items. "
        "If no, return mode=single with a cleaned content field and a concise summary. "
        "Always keep formatting minimal: paragraphs, lists, and links only."
    )

    user_prompt = "Subject: " + subject + "\nFrom: " + from_address + "\n\nContent:\n" + content

    schema = {
        "name": "newsletter_parse",
        "schema": {
            "type": "object",
            "properties": {
                "mode": {"type": "string", "enum": ["single", "multi"]},
                "summary": {"type": "string"},
                "content": {"type": "string"},
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": {"type": "string"},
                            "url": {"type": "string"},
                            "summary": {"type": "string"},
                            "content": {"type": "string"},
                        },
                        "required": ["url"],
                        "additionalProperties": False,
                    },
                },
            },
            "required": ["mode"],
            "additionalProperties": False,
        },
    }

    return {
        "model": options.openai_model,
        "messages": [
            {"role": "system", "content": instructions},
            {"role": "user", "content": user_prompt},
        ],
        "response_format": {"type": "json_schema", "json_schema": schema},
    }


def _extract_openai_response_text(response) -> str | None:
    if response is None:
        return None

    choices = getattr(response, "choices", [])
    if choices and len(choices) > 0:
        message = getattr(choices[0], "message", None)
        if message:
            return getattr(message, "content", None)

    return None


def _normalize_llm_result(result: dict) -> dict:
    normalized: dict = {"mode": result.get("mode")}
    if result.get("summary"):
        normalized["summary"] = result.get("summary")
    if result.get("content"):
        normalized["content"] = result.get("content")

    items = result.get("items")
    if isinstance(items, list):
        cleaned_items = []
        for item in items:
            if not isinstance(item, dict):
                continue
            url = item.get("url")
            if not url:
                continue
            cleaned_items.append(
                {
                    "title": item.get("title"),
                    "url": url,
                    "summary": item.get("summary"),
                    "content": item.get("content"),
                }
            )
        if cleaned_items:
            normalized["items"] = cleaned_items
    return normalized


def clean_up_old_newsletters(now_ts: int | None = None) -> int:
    """Remove old newsletter articles that are read, unstarred, and stale.

    :param now_ts: Optional timestamp override (primarily for testing).
    :returns: Number of articles removed.
    """

    current_time = now_ts if now_ts is not None else int(time.time())
    ninety_days_seconds = 90 * 24 * 60 * 60
    cutoff = current_time - ninety_days_seconds

    with get_session() as session:
        articles_to_delete = (
            session.query(database.Article)
            .join(database.Feed, database.Feed.id == database.Article.feed_id)
            .filter(database.Feed.is_mailing_list == True)  # noqa: E712
            .filter(database.Article.last_modified < cutoff)
            .filter(database.Article.unread == False)  # noqa: E712
            .filter(database.Article.starred == False)  # noqa: E712
            .all()
        )

        removed = len(articles_to_delete)

        if removed:
            logger.info(f"Removing {removed} old newsletter articles from database")

        for stale_article in articles_to_delete:
            session.delete(stale_article)

        session.commit()

    return removed
