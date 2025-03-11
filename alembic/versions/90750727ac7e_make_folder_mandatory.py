"""Make folder mandatory

Revision ID: 90750727ac7e
Revises: da62f26bd3e5
Create Date: 2025-03-11 16:58:25.670773

"""

from collections.abc import Sequence

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "90750727ac7e"
down_revision: str | None = "da62f26bd3e5"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    op.alter_column("folder", "name", existing_type=sa.VARCHAR(), nullable=True)
    op.execute("INSERT INTO folder (id, name) VALUES (0, NULL)")

    op.execute("UPDATE feed SET folder_id = 0 WHERE folder_id IS NULL")
    op.alter_column("feed", "folder_id", existing_type=sa.INTEGER(), nullable=False)


def downgrade() -> None:
    op.alter_column("feed", "folder_id", existing_type=sa.INTEGER(), nullable=True)
    op.execute("UPDATE feed SET folder_id = NULL WHERE folder_id = 0")

    op.execute("DELETE FROM folder WHERE id = 0")
    op.alter_column("folder", "name", existing_type=sa.VARCHAR(), nullable=False)
