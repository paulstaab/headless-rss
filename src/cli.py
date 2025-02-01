import click
from src.feed import update_all

@click.command()
def update_all_feeds():
    """Update all feeds in the database."""
    update_all()

if __name__ == "__main__":
    update_all_feeds()
