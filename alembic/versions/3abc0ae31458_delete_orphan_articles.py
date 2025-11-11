"""Delete orphaned articles

Revision ID: 3abc0ae31458
Revises: 70f699292c8b
Create Date: 2025-11-11 19:51:48.365034

"""

from typing import TYPE_CHECKING

from alembic import op

if TYPE_CHECKING:
    from collections.abc import Sequence

# revision identifiers, used by Alembic.
revision: str = "3abc0ae31458"
down_revision: str | None = "70f699292c8b"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    op.execute("""
        DELETE FROM article
        WHERE feed_id NOT IN (SELECT id FROM feed)
    """)


def downgrade() -> None:
    pass
