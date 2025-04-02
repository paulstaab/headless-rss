# headless-rss

A minimal-viable feed aggregator made for self-hosting.

`headless-rss` is an rss feed aggregator with a minimal feature set.
It is intended to be used in a homelab setting with a single user. It provides
a [Nextcloud News compatible API](https://github.com/nextcloud/news/blob/master/docs/api/api-v1-3.md),
and can serve as a back-end for compatible apps such as [NextNews](https://nextnewsapp.com) and
[Fiery Feeds](https://voidstern.net/fiery-feeds).

If you are looking for a feature-complete feed aggregator back-end that supports e.g.
different protocols, multiple users or databases other than sqlite, please look at
[Arsse](https://code.mensbeam.com/MensBeam/Arsse) or [Nextcloud News](https://apps.nextcloud.com/apps/news).


## Features

- [x] Can aggregate rss and atom feeds
- [x] Can serve as a back-end for applications that speak the Nextcloud News protocol.
      Fiery Feeds is tested and works well.
- [x] Can be hosted with a single unprivileged docker container.


## Usage Instructions
Start the API with:
```
docker run -d --rm --user 9999 --init \
  --name headless_rss \
  --volume headless-rss-data:/app/data \
  --publish 8000:8000 \
  --env USERNAME=myuser \
  --env PASSWORD=mypassword \
  ghcr.io/paulstaab/headless-rss:latest
```

The `USERNAME` and `PASSWORD` environment variables are optional to enable authentication.

The `FEED_UPDATE_FREQUENCY_MIN` environment variable is optional to set the update frequency for feeds
in minutes (default is 15 minutes).

## CLI Command to Connect to Email Account

A new CLI command `connect-email` has been added to connect to an email account and automatically detect newsletters by domain or mailing list id, adding them as feeds to the default folder while ignoring non-newsletter emails.

### Usage

```
python src/cli.py connect-email --email-address <email_address> --password <password> --protocol <IMAP|POP3> --server <server_address>
```

### Example

```
python src/cli.py connect-email --email-address example@gmail.com --password mypassword --protocol IMAP
```

## Contribution Guidelines

- Bugfixes are welcome.
- Please submit an issue for feature requests before creating a pull-request.
  I want to keep this project as small as possible.


## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
