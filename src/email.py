import imaplib
import poplib
import email
from email.header import decode_header
from typing import List, Tuple
from src import feed, folder, article, database
from sqlalchemy.orm import Session

# Function to connect to an email account using IMAP
def connect_imap(email_address: str, password: str, server: str) -> imaplib.IMAP4_SSL:
    mail = imaplib.IMAP4_SSL(server)
    mail.login(email_address, password)
    store_email_credentials(email_address, password, server, "IMAP")
    return mail

# Function to connect to an email account using POP3
def connect_pop3(email_address: str, password: str, server: str) -> poplib.POP3_SSL:
    mail = poplib.POP3_SSL(server)
    mail.user(email_address)
    mail.pass_(password)
    store_email_credentials(email_address, password, server, "POP3")
    return mail

# Function to detect newsletters based on specified criteria
def detect_newsletters(mail) -> List[email.message.Message]:
    newsletters = []
    mail.select("inbox")
    result, data = mail.search(None, "ALL")
    email_ids = data[0].split()
    for email_id in email_ids:
        result, msg_data = mail.fetch(email_id, "(RFC822)")
        msg = email.message_from_bytes(msg_data[0][1])
        if is_newsletter(msg):
            newsletters.append(msg)
    return newsletters

# Helper function to check if an email is a newsletter
def is_newsletter(msg: email.message.Message) -> bool:
    sender_domain = msg["From"].split("@")[-1]
    subject = decode_header(msg["Subject"])[0][0]
    if isinstance(subject, bytes):
        subject = subject.decode()
    body = get_email_body(msg)
    if (
        sender_domain in known_newsletter_domains
        or "newsletter" in subject.lower()
        or "weekly update" in subject.lower()
        or "subscription" in subject.lower()
        or "List-ID" in msg
        or "List-Unsubscribe" in msg
        or "List-Archive" in msg
        or "tracking pixel" in body
    ):
        return True
    return False

# Helper function to get the email body
def get_email_body(msg: email.message.Message) -> str:
    if msg.is_multipart():
        for part in msg.walk():
            if part.get_content_type() == "text/plain":
                return part.get_payload(decode=True).decode()
    else:
        return msg.get_payload(decode=True).decode()
    return ""

# Function to add detected newsletters as feeds to the default folder
def add_newsletters_as_feeds(newsletters: List[email.message.Message]) -> None:
    default_folder_id = folder.get_root_folder_id()
    for newsletter in newsletters:
        feed_url = extract_feed_url(newsletter)
        if feed_url:
            feed.add(feed_url, default_folder_id)

# Helper function to extract feed URL from a newsletter
def extract_feed_url(newsletter: email.message.Message) -> str:
    body = get_email_body(newsletter)
    # Extract feed URL from the body (implementation depends on the email structure)
    return ""

# Function to check for new emails to existing newsletter feeds and add them as articles
def check_for_new_emails(mail) -> None:
    newsletters = detect_newsletters(mail)
    for newsletter in newsletters:
        feed_url = extract_feed_url(newsletter)
        if feed_url:
            existing_feed = feed.get_by_url(feed_url)
            if existing_feed:
                article.add_from_email(newsletter, existing_feed.id)

# Helper function to get feed by URL
def get_feed_by_url(url: str) -> feed.Feed:
    feeds = feed.get_all()
    for f in feeds:
        if f.url == url:
            return f
    return None

# Function to store email credentials in the database
def store_email_credentials(email_address: str, password: str, server: str, protocol: str) -> None:
    with database.get_session() as db:
        credentials = EmailCredentials(
            email_address=email_address,
            password=password,
            server=server,
            protocol=protocol
        )
        db.add(credentials)
        db.commit()

# EmailCredentials model
class EmailCredentials(database.Base):
    __tablename__ = "email_credentials"

    id = database.mapped_column(database.Integer, primary_key=True)
    email_address = database.mapped_column(database.String, unique=True, nullable=False)
    password = database.mapped_column(database.String, nullable=False)
    server = database.mapped_column(database.String, nullable=False)
    protocol = database.mapped_column(database.String, nullable=False)
