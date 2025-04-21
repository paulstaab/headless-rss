import imaplib
import logging
from email.header import decode_header
from email.parser import BytesParser

from src import article, database, feed, folder
from src.database import EmailCredential, get_session

logger = logging.getLogger(__name__)


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
        return
    for credential in credentials:
        fetch_emails(credential)


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
            except (LookupError, UnicodeDecodeError):
                content = content.decode("utf-8", errors="replace")  # Fallback
        elif not isinstance(content, str):
            content = str(content)  # Convert other types to string

        new_article = _create_article_from_email(
            feed_id=feed_id,
            subject=subject,
            from_address=from_address,
            content=content,
        )
        existing_article = session.query(database.Article).filter_by(guid_hash=new_article.guid_hash).first()
        if not existing_article:
            session.add(new_article)
            session.commit()
            logger.info(f"Added email '{subject}' to feed '{feed_title}'")


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
) -> database.Article:
    """Create an article from email data."""
    guid = from_address + ":" + subject

    new_article = article.create(
        feed_id=feed_id, title=subject, author=from_address, content=content, guid=guid, url=None
    )

    return new_article
