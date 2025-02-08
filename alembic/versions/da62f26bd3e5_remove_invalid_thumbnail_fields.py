"""Remove invalid thumbnail fields

Revision ID: da62f26bd3e5
Revises: 04239faf8d5b
Create Date: 2025-02-08 14:51:32.548474

"""

from collections.abc import Sequence

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "da62f26bd3e5"
down_revision: str | None = "04239faf8d5b"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    op.execute("UPDATE article SET media_description = NULL, media_thumbnail = NULL")


def downgrade() -> None:
    pass
