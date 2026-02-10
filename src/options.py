from __future__ import annotations

import os
import sys
from dataclasses import dataclass
from typing import ClassVar

DEFAULT_FEED_UPDATE_FREQUENCY_MIN = 15
DEFAULT_VERSION = "dev"
DEFAULT_OPENAI_MODEL = "gpt-5-mini"


def _get_env_str(name: str) -> str | None:
    value = os.getenv(name)
    if value is None:
        return None
    value = value.strip()
    return value or None


def _get_env_int(name: str, default: int) -> int:
    raw_value = os.getenv(name)
    if raw_value is None:
        return default
    try:
        return int(raw_value)
    except ValueError:
        return default


@dataclass(frozen=True)
class Options:
    username: str | None
    password: str | None
    feed_update_frequency_min: int
    version: str
    openai_api_key: str | None
    openai_model: str

    _instance: ClassVar[Options | None] = None

    @classmethod
    def get(cls) -> Options:
        if cls._instance is None:
            cls._instance = cls._from_env()
        return cls._instance

    @classmethod
    def clear(cls) -> None:
        cls._instance = None

    @classmethod
    def _from_env(cls) -> Options:
        return cls(
            username=_get_env_str("USERNAME"),
            password=_get_env_str("PASSWORD"),
            feed_update_frequency_min=_get_env_int("FEED_UPDATE_FREQUENCY_MIN", DEFAULT_FEED_UPDATE_FREQUENCY_MIN),
            version=os.getenv("VERSION", DEFAULT_VERSION),
            openai_api_key=_get_env_str("OPENAI_API_KEY"),
            openai_model=os.getenv("OPENAI_MODEL", DEFAULT_OPENAI_MODEL),
        )

    @property
    def llm_enabled(self) -> bool:
        return bool(self.openai_api_key)

    @property
    def testing_mode(self) -> bool:
        """Detect if we're running in testing mode."""
        return "pytest" in sys.modules or any("test" in module for module in sys.modules)
