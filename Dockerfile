ARG python_version=3.13

FROM ghcr.io/astral-sh/uv:python${python_version}-bookworm-slim AS builder
ENV UV_COMPILE_BYTECODE=1 UV_LINK_MODE=copy
WORKDIR /app
RUN --mount=type=cache,target=/root/.cache/uv \
    --mount=type=bind,source=uv.lock,target=uv.lock \
    --mount=type=bind,source=pyproject.toml,target=pyproject.toml \
    uv sync --frozen --no-install-project --no-dev
ADD . /app
RUN --mount=type=cache,target=/root/.cache/uv \
    uv sync --frozen --no-dev

FROM python:${python_version}-slim-bookworm
COPY --from=builder --chown=app:app /app /app
COPY --chown=app:app ./docker/entrypoint.sh /app/entrypoint.sh
ENV PATH="/app/.venv/bin:$PATH"
WORKDIR /app
ENTRYPOINT ["/app/entrypoint.sh"]
CMD ["start"]
