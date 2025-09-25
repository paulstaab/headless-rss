# Email Newsletter Conversion

## Overview
- Transform mailing list emails into RSS articles stored in the local SQLite database.
- Run automatically from `feed.update_all()` after standard feed polling and from the CLI `update` command.

## Credential Management
- Credentials are stored via `add_credentials(protocol, server, port, username, password)`.
- Only IMAP over SSL is supported (`protocol == "imap"`); other protocols raise `NotImplementedError`.
- Credentials are validated on insertion by attempting to connect and select the inbox. Failures raise `EmailConnectionError`.

## CLI for Adding Email Accounts
- Run the CLI subcommand `add-email-credentials --server <host> --port <port> --username <user> --password <pass>` to register a mailbox.
- The command initialises the SQLite database at `data/headless-rss.sqlite3` before storing credentials.
- Each option is required; invalid connections raise an exception with the underlying connection error message.
- Successful executions echo "Email credentials added successfully." and the mailbox becomes eligible for the next update cycle.
- Invoke the command multiple times to add additional IMAP accounts; existing credentials remain untouched.

## Mailbox Processing
- Each configured mailbox is polled for `UNSEEN` messages.
- Fetched messages are marked as seen after successful processing to avoid duplication.
- Errors during search or fetch are logged, and the message is skipped without interrupting the loop.

## Mailing List Identification and Feed Lifecycle
- Messages must present a `List-Unsubscribe` header to qualify as mailing list newsletters.
- The sender address becomes the feed URL; the display name or domain-derived label becomes the feed title.
- If no feed exists for the address, one is created in the root folder with `is_mailing_list=True`.

## Content Extraction and Cleanup
- HTML bodies are preferred; plain text is used as a fallback.
- Content is decoded using the message charset or UTF-8 with replacement on failure.
- HTML bodies pass through `NewsletterHTMLCleaner`, which:
  - Removes hidden sections, tracking pixels, layout tables, and meta tags.
  - Preserves semantic structure (headings, lists, paragraphs) while stripping non-essential attributes.
  - Collapses excessive whitespace and returns cleaned markup; empty or `None` inputs yield an empty string.

## Article Construction and Deduplication
- Articles use the email subject as the title, the sender as the author, and the cleaned body as the content.
- GUIDs are formed as `<from_address>:<subject>`; the corresponding MD5 hash prevents duplicates.
- Prior to insertion, the database is checked for an existing article with the same `guid_hash`; duplicates are discarded.

## Error Handling and Logging
- Non-mailing list emails are ignored with an informational log entry.
- HTML cleaning failures fall back to minimal regex-based sanitization and emit a warning.
- Connection and parsing issues are logged with context but do not halt batch processing.
