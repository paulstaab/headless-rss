"""Create initial db model

Revision ID: 936806105f66
Revises:
Create Date: 2025-02-01 13:27:55.795887

"""

from collections.abc import Sequence

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "936806105f66"
down_revision: str | None = None
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    """Create initial database schema.

    This function creates the initial database schema with tables for folders, feeds, and articles.
    """
    op.create_table(
        "folder",
        sa.Column("id", sa.Integer(), nullable=False),
        sa.Column("name", sa.String(), nullable=False),
        sa.PrimaryKeyConstraint("id"),
        sa.UniqueConstraint("name"),
    )
    op.create_table(
        "feed",
        sa.Column("id", sa.Integer(), nullable=False),
        sa.Column("url", sa.String(), nullable=False),
        sa.Column("title", sa.String(), nullable=True),
        sa.Column("favicon_link", sa.String(), nullable=True),
        sa.Column("added", sa.Integer(), nullable=False),
        sa.Column("next_update_time", sa.Integer(), nullable=True),
        sa.Column("folder_id", sa.Integer(), nullable=False),
        sa.Column("ordering", sa.Integer(), nullable=False),
        sa.Column("link", sa.String(), nullable=True),
        sa.Column("pinned", sa.Boolean(), nullable=False),
        sa.Column("update_error_count", sa.Integer(), nullable=False),
        sa.Column("last_update_error", sa.String(), nullable=True),
        sa.ForeignKeyConstraint(
            ["folder_id"],
            ["folder.id"],
        ),
        sa.PrimaryKeyConstraint("id"),
        sa.UniqueConstraint("url"),
    )
    op.create_table(
        "article",
        sa.Column("id", sa.Integer(), nullable=False),
        sa.Column("title", sa.String(), nullable=True),
        sa.Column("content", sa.String(), nullable=True),
        sa.Column("author", sa.String(), nullable=True),
        sa.Column("body", sa.String(), nullable=True),
        sa.Column("content_hash", sa.String(), nullable=True),
        sa.Column("enclosure_link", sa.String(), nullable=True),
        sa.Column("enclosure_mime", sa.String(), nullable=True),
        sa.Column("feed_id", sa.Integer(), nullable=False),
        sa.Column("fingerprint", sa.String(), nullable=True),
        sa.Column("guid", sa.String(), nullable=False),
        sa.Column("guid_hash", sa.String(), nullable=False),
        sa.Column("last_modified", sa.Integer(), nullable=False),
        sa.Column("media_description", sa.String(), nullable=True),
        sa.Column("media_thumbnail", sa.String(), nullable=True),
        sa.Column("pub_date", sa.Integer(), nullable=True),
        sa.Column("rtl", sa.Boolean(), nullable=False),
        sa.Column("starred", sa.Boolean(), nullable=False),
        sa.Column("unread", sa.Boolean(), nullable=False),
        sa.Column("updated_date", sa.Integer(), nullable=True),
        sa.Column("url", sa.String(), nullable=True),
        sa.ForeignKeyConstraint(
            ["feed_id"],
            ["feed.id"],
        ),
        sa.PrimaryKeyConstraint("id"),
    )


def downgrade() -> None:
    """Drop all tables.

    This function drops all tables created in the upgrade function, effectively reverting
    the database schema to its initial state.
    """
    op.drop_table("article")
    op.drop_table("feed")
    op.drop_table("folder")
