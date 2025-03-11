"""Add default root folder

Revision ID: 01f73e6bfaea
Revises: da62f26bd3e5
Create Date: 2025-03-11 17:24:10.645039

"""

from collections.abc import Sequence

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "01f73e6bfaea"
down_revision: str | None = "da62f26bd3e5"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    op.add_column("folder", sa.Column("is_root", sa.Boolean(), nullable=False))
    op.execute("INSERT INTO folder (id, name, is_root) VALUES (0, '', true)")
    op.execute("UPDATE feed SET folder_id = 0 WHERE folder_id IS NULL")


def downgrade() -> None:
    raise NotImplementedError("Downgrade is not supported for this migration.")
