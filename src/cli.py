import click

from src.feed import update_all


@click.command()
@click.argument("command", required=True, help="Command to run")
def cli(command):
    if command == "update":
        update_all()


if __name__ == "__main__":
    cli()
