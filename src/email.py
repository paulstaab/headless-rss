import imaplib
import logging
import poplib
from email.parser import BytesParser

from src import database, feed, folder
from src.database import EmailCredential, get_session

logger = logging.getLogger(__name__)


def add_credentials(protocol, server, port, username, password):
    """Store email credentials in the database."""
    logger.info(f"Adding email credentials for user {username} on server {server}")
    with get_session() as session:
        credential = EmailCredential(protocol=protocol, server=server, port=port, username=username, password=password)
        session.add(credential)
        session.commit()
    logger.info(f"Successfully added email credentials for user {username}")


def fetch_emails() -> None:  # noqa: C901
    """Fetch emails from all configured mailboxes."""
    try:
        with get_session() as session:
            credentials = session.query(EmailCredential).all()
            if not credentials:
                logger.info("No email credentials configured. Skipping email fetch.")
                return

            for credential in credentials:
                logger.info(
                    f"Fetching emails for {credential.username} "
                    f"via {credential.protocol.upper()} from {credential.server}:{credential.port}"
                )
                try:
                    if credential.protocol.lower() == "imap":
                        mail = imaplib.IMAP4_SSL(credential.server, credential.port)  # type: ignore
                        mail.login(credential.username, credential.password)  # type: ignore
                        mail.select("inbox")
                        logger.debug("IMAP: Logged in and selected inbox.")
                        status, messages = mail.search(None, "ALL")
                        if status != "OK":
                            logger.error(f"IMAP: Failed search for {credential.username}. Status: {status}")
                            continue
                        email_ids = messages[0].split()
                        logger.info(f"IMAP: Found {len(email_ids)} emails for {credential.username}.")
                        for num in email_ids:
                            fetch_status, data = mail.fetch(num, "(RFC822)")
                            if fetch_status == "OK" and data and data[0] is not None:
                                raw_email = data[0][1]
                                process_email(raw_email)
                            else:
                                logger.warning(
                                    f"IMAP: Failed fetch email ID {num.decode()} for "
                                    f"{credential.username}. Status: {fetch_status}"
                                )
                        mail.logout()
                        logger.debug(f"IMAP: Logged out for {credential.username}.")

                    elif credential.protocol.lower() == "pop3":
                        mail_pop = poplib.POP3_SSL(credential.server, credential.port)  # type: ignore
                        mail_pop.user(credential.username)  # type: ignore
                        mail_pop.pass_(credential.password)  # type: ignore
                        logger.debug("POP3: Logged in.")
                        num_messages = len(mail_pop.list()[1])
                        logger.info(f"POP3: Found {num_messages} emails for {credential.username}.")
                        for i in range(num_messages):
                            try:
                                response, lines, octets = mail_pop.retr(i + 1)
                                if response.startswith(b"+OK"):
                                    raw_email = b"\n".join(lines)
                                    process_email(raw_email)
                                else:
                                    logger.warning(
                                        f"POP3: Failed retrieve email index {i + 1} for "
                                        f"{credential.username}. Response: {response.decode()}"
                                    )
                            except Exception as e:
                                logger.error(
                                    f"POP3: Error processing email index {i + 1} for {credential.username}: {e}"
                                )
                        mail_pop.quit()
                        logger.debug(f"POP3: Connection closed for {credential.username}.")
                except Exception as e:
                    logger.error(
                        f"Error fetching emails for {credential.username} from {credential.server}: {e}", exc_info=True
                    )

    except Exception as e:
        logger.error(f"General error during email fetching process: {e}", exc_info=True)
    logger.info("Finished email fetch process.")


def process_email(raw_email) -> None:  # noqa: C901
    """Process a raw email message."""
    try:
        msg = BytesParser().parsebytes(raw_email)
        subject = msg["subject"]
        from_address = msg["from"]
        logger.debug(f"Processing email: Subject='{subject}', From='{from_address}'")

        # Basic check if the email is from a mailing list (customize this logic)
        # Look for headers like List-Id, List-Unsubscribe, or specific patterns in From/To
        is_mailing_list = "List-Unsubscribe" in msg

        if is_mailing_list:
            # Use List-Id or a cleaned version of From as feed title
            feed_title = from_address.split("@")[0]
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
                        if content_type == "text/plain" and "attachment" not in content_disposition:
                            content = part.get_payload(decode=True)  # type: ignore
                            break  # Prefer plain text
                    if not content:  # Fallback to HTML if no plain text
                        for part in msg.walk():
                            content_type = part.get_content_type()
                            content_disposition = str(part.get("Content-Disposition"))
                            if content_type == "text/html" and "attachment" not in content_disposition:
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

                # Add email as an article to the feed
                # Need to adapt feed.add_article or create a similar function
                # Placeholder for adding article logic
                logger.info(f"Adding article '{subject}' to feed ID {feed_id}")
                # feed.add_article(feed_id, title=subject, content=content, ...) # Needs implementation/adaptation
        else:
            logger.debug(f"Ignoring non-mailing list email: Subject='{subject}', From='{from_address}'")

    except Exception as e:
        logger.error(f"Error processing email: {e}", exc_info=True)
