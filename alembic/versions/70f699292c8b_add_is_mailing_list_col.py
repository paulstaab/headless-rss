"""Add is_mailing_list column to feed table

Revision ID: 70f699292c8b
Revises: f0a4c607411b
Create Date: 2025-04-21 14:16:34.497856

"""

from typing import TYPE_CHECKING

import sqlalchemy as sa
from alembic import op

if TYPE_CHECKING:
    from collections.abc import Sequence

# revision identifiers, used by Alembic.
revision: str = "70f699292c8b"
down_revision: str | None = "f0a4c607411b"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    op.add_column("feed", sa.Column("is_mailing_list", sa.Boolean(), nullable=False, server_default=sa.false()))


def downgrade() -> None:
    op.drop_column("feed", "is_mailing_list")
