"""Add feed quality flags

Revision ID: 5c2f1d0f3b1a
Revises: f0a4c607411b
Create Date: 2026-02-09 00:00:00.000000

"""

from typing import TYPE_CHECKING

import sqlalchemy as sa
from alembic import op

if TYPE_CHECKING:
    from collections.abc import Sequence

# revision identifiers, used by Alembic.
revision: str = "5c2f1d0f3b1a"
down_revision: str | None = "3abc0ae31458"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    op.add_column("feed", sa.Column("last_quality_check", sa.Integer(), nullable=True))
    op.add_column(
        "feed",
        sa.Column("use_extracted_fulltext", sa.Boolean(), nullable=False, server_default=sa.false()),
    )
    op.add_column(
        "feed",
        sa.Column("use_llm_summary", sa.Boolean(), nullable=False, server_default=sa.false()),
    )


def downgrade() -> None:
    op.drop_column("feed", "use_llm_summary")
    op.drop_column("feed", "use_extracted_fulltext")
    op.drop_column("feed", "last_quality_check")
