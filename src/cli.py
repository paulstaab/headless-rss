import logging
from pathlib import Path

import click

from src import database, email, feed


@click.group()
def cli():
    """CLI for headless-rss."""
    pass


@cli.command()
def update() -> None:
    """Execute the specified command.

    :param command: The command to execute. Supported commands: 'update'.
    """
    database.init(Path("data/headless-rss.sqlite3"))
    feed.update_all()


@cli.command()
@click.option("--server", required=True, help="Email server address.")
@click.option("--port", required=True, type=int, help="Port number for the email server.")
@click.option("--username", required=True, help="Email username.")
@click.option("--password", required=True, help="Email password.")
def add_email_credentials(server, port, username, password) -> None:
    """Add email credentials for fetching newsletters."""
    database.init(Path("data/headless-rss.sqlite3"))
    try:
        email.add_credentials(protocol="imap", server=server, port=port, username=username, password=password)
    except Exception as e:
        raise click.ClickException(f"Failed to add email credentials: {e}") from None
    click.echo("Email credentials added successfully.")


if __name__ == "__main__":
    logging.basicConfig(level=logging.WARNING)
    cli()
