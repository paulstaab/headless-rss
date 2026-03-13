PRAGMA foreign_keys=OFF;

CREATE TABLE article_new (
    id INTEGER PRIMARY KEY NOT NULL,
    title VARCHAR,
    content VARCHAR,
    author VARCHAR,
    summary VARCHAR,
    content_hash VARCHAR,
    enclosure_link VARCHAR,
    enclosure_mime VARCHAR,
    feed_id INTEGER NOT NULL,
    fingerprint VARCHAR,
    guid VARCHAR NOT NULL,
    guid_hash VARCHAR NOT NULL,
    last_modified INTEGER NOT NULL,
    media_description VARCHAR,
    media_thumbnail VARCHAR,
    pub_date INTEGER,
    rtl BOOLEAN NOT NULL DEFAULT 0,
    starred BOOLEAN NOT NULL DEFAULT 0,
    unread BOOLEAN NOT NULL DEFAULT 1,
    updated_date INTEGER,
    url VARCHAR,
    FOREIGN KEY(feed_id) REFERENCES feed(id)
);

INSERT INTO article_new (
    id,
    title,
    content,
    author,
    summary,
    content_hash,
    enclosure_link,
    enclosure_mime,
    feed_id,
    fingerprint,
    guid,
    guid_hash,
    last_modified,
    media_description,
    media_thumbnail,
    pub_date,
    rtl,
    starred,
    unread,
    updated_date,
    url
)
SELECT
    id,
    title,
    content,
    author,
    summary,
    content_hash,
    enclosure_link,
    enclosure_mime,
    feed_id,
    fingerprint,
    guid,
    guid_hash,
    last_modified,
    media_description,
    media_thumbnail,
    pub_date,
    rtl,
    starred,
    unread,
    updated_date,
    url
FROM article;

DROP TABLE article;
ALTER TABLE article_new RENAME TO article;

PRAGMA foreign_keys=ON;
