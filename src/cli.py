import logging
from pathlib import Path

import click

from src import database, email, feed


@click.group()
def cli():
    """CLI for headless-rss."""
    pass


@cli.command()
@click.argument("command", required=True)
def execute(command):
    """Execute the specified command.

    :param command: The command to execute. Supported commands: 'update'.
    """
    database.init(Path("data/headless-rss.sqlite3"))
    if command == "update":
        feed.update_all()


@cli.command()
@click.option(
    "--protocol",
    type=click.Choice(["imap", "pop3"], case_sensitive=False),
    required=True,
    help="Protocol to use (IMAP or POP3).",
)
@click.option("--server", required=True, help="Email server address.")
@click.option("--port", required=True, type=int, help="Port number for the email server.")
@click.option("--username", required=True, help="Email username.")
@click.option("--password", required=True, help="Email password.")
def add_email_credentials(protocol, server, port, username, password):
    """Add email credentials for fetching newsletters."""
    email.add_credentials(protocol, server, port, username, password)
    click.echo("Email credentials added successfully.")


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    cli()
