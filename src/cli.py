import logging
from pathlib import Path

import click

from src import database, feed


@click.command()
@click.argument("command", required=True)
def cli(command):
    """Execute the specified command.

    :param command: The command to execute. Supported commands: 'update'.
    """
    database.init(Path("data/headless-rss.sqlite3"))
    if command == "update":
        feed.update_all()


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    cli()
