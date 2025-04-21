"""Add email_credentials table and is_mailing_list column to feed

Revision ID: f0a4c607411b
Revises: 01f73e6bfaea
Create Date: 2025-04-21 11:54:44.462605

"""

from collections.abc import Sequence

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "f0a4c607411b"
down_revision: str | None = "01f73e6bfaea"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    op.create_table(
        "email_credentials",
        sa.Column("id", sa.Integer(), autoincrement=True, nullable=False),
        sa.Column("protocol", sa.String(), nullable=False),
        sa.Column("server", sa.String(), nullable=False),
        sa.Column("port", sa.Integer(), nullable=False),
        sa.Column("username", sa.String(), nullable=False),
        sa.Column("password", sa.String(), nullable=False),
        sa.PrimaryKeyConstraint("id"),
    )
    op.add_column("feed", sa.Column("is_mailing_list", sa.Boolean(), nullable=False))


def downgrade() -> None:
    op.drop_column("feed", "is_mailing_list")
    op.drop_table("email_credentials")
