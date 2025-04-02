import logging
from pathlib import Path

import click

from src import database, feed, email


@click.command()
@click.argument("command", required=True)
@click.option("--email-address", help="Email address to connect to")
@click.option("--password", help="Password for the email account")
@click.option("--protocol", type=click.Choice(["IMAP", "POP3"]), help="Email protocol to use (IMAP or POP3)")
@click.option("--server", help="Email server to connect to")
def cli(command, email_address, password, protocol, server):
    """Execute the specified command.

    :param command: The command to execute. Supported commands: 'update', 'connect-email'.
    :param email_address: The email address to connect to.
    :param password: The password for the email account.
    :param protocol: The email protocol to use (IMAP or POP3).
    :param server: The email server to connect to.
    """
    database.init(Path("data/headless-rss.sqlite3"))
    if command == "update":
        feed.update_all()
    elif command == "connect-email":
        if not email_address or not password or not protocol or not server:
            raise click.UsageError("Email address, password, protocol, and server are required for 'connect-email' command")
        if protocol == "IMAP":
            mail = email.connect_imap(email_address, password, server)
        elif protocol == "POP3":
            mail = email.connect_pop3(email_address, password, server)
        newsletters = email.detect_newsletters(mail)
        email.add_newsletters_as_feeds(newsletters)
        email.check_for_new_emails(mail)


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    cli()
