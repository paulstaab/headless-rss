# headless-rss

A minimal-viable feed aggregator made for self-hosting.

`headless-rss` is an rss feed aggregator with a minimal feature set.
It is indended to be used in a homelab setting with a single user. It provides
a [Nextcloud News compatible API](https://github.com/nextcloud/news/blob/master/docs/api/api-v1-3.md),
and can serve as a back-end for compatible apps suchs as [NextNews](https://nextnewsapp.com) and
[Fiery Feeds](https://voidstern.net/fiery-feeds).

If you are looking for a feature-complete feed aggregator back-end that supports e.g.
different protocols, multiple users or databases other than sqlite, please look at
[Arsse](https://code.mensbeam.com/MensBeam/Arsse) or [Nextcloud News](https://apps.nextcloud.com/apps/news).

## Features

- [ ] Can aggregate rss and atom feeds
- [ ] Can serve as a back-end for applications that speak the Nextcloud News protocol
- [ ] Can be hosted with a single unpriviledged docker container


## Usage Instructions
Start the API with:
```
docker run -d --rm --user 9999 --init \
  --name headless_rss \
  --volume headless-rss-data:/app/data \
  --publish 8000:8000 \
  ghcr.io/paulstaab/headless-rss:latest
```

Additionally, you need to setup some kind of cronjob for
```
docker exec -ti headless_rss /app/docker/entrypoint update
```
to collect the feeds regularly.

## Authentication

The API uses HTTP basic authentication. To enable authentication, set the `USERNAME` and `PASSWORD` environment variables when creating the docker container. The credentials need to be passed as an HTTP header using HTTP basic auth:

```
Authorization: Basic $CREDENTIALS
```

where `$CREDENTIALS` is: `base64(USER:PASSWORD)`

### Example

Start the container with authentication enabled:
```
docker run -d --rm --user 9999 --init \
  --name headless_rss \
  --volume headless-rss-data:/app/data \
  --publish 8000:8000 \
  -e USERNAME=myuser \
  -e PASSWORD=mypassword \
  ghcr.io/paulstaab/headless-rss:latest
```

## Contribution Guidelines

- Bugfixes are welcome.
- Please submit an issue for feature requests before creating a pull-request.
  I want to keep this project as small as possible.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
