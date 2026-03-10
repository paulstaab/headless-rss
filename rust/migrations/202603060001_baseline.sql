CREATE TABLE IF NOT EXISTS folder (
    id INTEGER PRIMARY KEY NOT NULL,
    name VARCHAR NOT NULL UNIQUE,
    is_root BOOLEAN NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS feed (
    id INTEGER PRIMARY KEY NOT NULL,
    url VARCHAR NOT NULL UNIQUE,
    title VARCHAR,
    favicon_link VARCHAR,
    added INTEGER NOT NULL,
    next_update_time INTEGER,
    folder_id INTEGER NOT NULL,
    ordering INTEGER NOT NULL DEFAULT 0,
    link VARCHAR,
    pinned BOOLEAN NOT NULL DEFAULT 0,
    update_error_count INTEGER NOT NULL DEFAULT 0,
    last_update_error VARCHAR,
    is_mailing_list BOOLEAN NOT NULL DEFAULT 0,
    last_quality_check INTEGER,
    use_extracted_fulltext BOOLEAN NOT NULL DEFAULT 0,
    use_llm_summary BOOLEAN NOT NULL DEFAULT 0,
    FOREIGN KEY(folder_id) REFERENCES folder(id)
);

CREATE TABLE IF NOT EXISTS article (
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
    unread BOOLEAN NOT NULL DEFAULT 0,
    updated_date INTEGER,
    url VARCHAR,
    FOREIGN KEY(feed_id) REFERENCES feed(id)
);

CREATE TABLE IF NOT EXISTS email_credentials (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    protocol VARCHAR NOT NULL,
    server VARCHAR NOT NULL,
    port INTEGER NOT NULL,
    username VARCHAR NOT NULL,
    password VARCHAR NOT NULL
);

INSERT INTO folder (id, name, is_root)
SELECT 0, '', 1
WHERE NOT EXISTS (SELECT 1 FROM folder WHERE id = 0);

UPDATE folder SET is_root = 1 WHERE id = 0;
UPDATE feed SET folder_id = 0 WHERE folder_id IS NULL;

DELETE FROM article
WHERE feed_id NOT IN (SELECT id FROM feed);
