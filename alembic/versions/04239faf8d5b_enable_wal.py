"""Enable WAL

Revision ID: 04239faf8d5b
Revises: 936806105f66
Create Date: 2025-02-02 08:15:02.419253

"""

from collections.abc import Sequence

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "04239faf8d5b"
down_revision: str | None = "936806105f66"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    op.execute("PRAGMA journal_mode=WAL")


def downgrade() -> None:
    op.execute("PRAGMA journal_mode=delete")
