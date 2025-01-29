"""add article model

Revision ID: 2023_09_15_123456
Revises: 
Create Date: 2023-09-15 12:34:56.789012

"""
from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision = '2023_09_15_123456'
down_revision = None
branch_labels = None
depends_on = None


def upgrade():
    op.create_table(
        'article',
        sa.Column('id', sa.Integer, primary_key=True),
        sa.Column('title', sa.String, nullable=True, default=None),
        sa.Column('content', sa.String, nullable=True, default=None),
        sa.Column('author', sa.String, nullable=True, default=None),
        sa.Column('body', sa.String, nullable=True, default=None),
        sa.Column('content_hash', sa.String, nullable=True, default=None),
        sa.Column('enclosure_link', sa.String, nullable=True, default=None),
        sa.Column('enclosure_mime', sa.String, nullable=True, default=None),
        sa.Column('feed_id', sa.Integer, sa.ForeignKey('feed.id')),
        sa.Column('fingerprint', sa.String, nullable=True, default=None),
        sa.Column('guid', sa.String, nullable=False),
        sa.Column('guid_hash', sa.String, nullable=False),
        sa.Column('last_modified', sa.String, nullable=True, default="0"),
        sa.Column('media_description', sa.String, nullable=True, default=None),
        sa.Column('media_thumbnail', sa.String, nullable=True, default=None),
        sa.Column('pub_date', sa.Integer, nullable=True, default=None),
        sa.Column('rtl', sa.Boolean, nullable=False, default=False),
        sa.Column('starred', sa.Boolean, nullable=False, default=False),
        sa.Column('unread', sa.Boolean, nullable=False, default=False),
        sa.Column('updated_date', sa.String, nullable=True, default=None),
        sa.Column('url', sa.String, nullable=True, default=None),
    )


def downgrade():
    op.drop_table('article')
