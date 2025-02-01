"""Update feed folder_id and folder name

Revision ID: 2025
Revises: 936806105f66
Create Date: 2025-02-01 14:00:00.000000

"""

from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision = '2025'
down_revision = '936806105f66'
branch_labels = None
depends_on = None


def upgrade():
    # Delete folder with id 0
    op.execute("DELETE FROM folder WHERE id = 0")

    # Update feeds to have folder_id as NULL
    op.execute("UPDATE feed SET folder_id = NULL WHERE folder_id = 0")

    # Update feed.folder_id to nullable=True
    op.alter_column('feed', 'folder_id', existing_type=sa.Integer(), nullable=True)

    # Update folder.name to nullable=False
    op.alter_column('folder', 'name', existing_type=sa.String(), nullable=False)


def downgrade():
    # Revert folder.name to nullable=True
    op.alter_column('folder', 'name', existing_type=sa.String(), nullable=True)

    # Revert feed.folder_id to nullable=False
    op.alter_column('feed', 'folder_id', existing_type=sa.Integer(), nullable=False)

    # Revert feeds with NULL folder_id to folder_id 0
    op.execute("UPDATE feed SET folder_id = 0 WHERE folder_id IS NULL")

    # Recreate folder with id 0
    op.execute("INSERT INTO folder (id, name) VALUES (0, 'Default')")
