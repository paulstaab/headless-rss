"""Rename body to summary

Revision ID: 8f3c1a2b9c7d
Revises: 5c2f1d0f3b1a
Create Date: 2026-02-10 00:00:00.000000

"""

from typing import TYPE_CHECKING

import sqlalchemy as sa
from alembic import op

if TYPE_CHECKING:
    from collections.abc import Sequence

# revision identifiers, used by Alembic.
revision: str = "8f3c1a2b9c7d"
down_revision: str | None = "5c2f1d0f3b1a"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    with op.batch_alter_table("article") as batch_op:
        batch_op.add_column(sa.Column("summary", sa.String(), nullable=True))

    op.execute("UPDATE article SET summary = body WHERE summary IS NULL")

    with op.batch_alter_table("article") as batch_op:
        batch_op.drop_column("body")


def downgrade() -> None:
    with op.batch_alter_table("article") as batch_op:
        batch_op.add_column(sa.Column("body", sa.String(), nullable=True))

    op.execute("UPDATE article SET body = summary WHERE body IS NULL")

    with op.batch_alter_table("article") as batch_op:
        batch_op.drop_column("summary")
