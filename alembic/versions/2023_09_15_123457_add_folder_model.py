"""add folder model

Revision ID: 2023_09_15_123457
Revises: 2023_09_15_123456
Create Date: 2023-09-15 12:34:57.789012

"""

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision = "2023_09_15_123457"
down_revision = "2023_09_15_123456"
branch_labels = None
depends_on = None


def upgrade():
    op.create_table(
        "folder",
        sa.Column("id", sa.Integer, primary_key=True),
        sa.Column("name", sa.String, nullable=False),
    )


def downgrade():
    op.drop_table("folder")
