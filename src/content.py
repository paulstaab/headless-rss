import json
import logging
import re

import trafilatura
from openai import OpenAI

from src.options import Options

logger = logging.getLogger(__name__)

OPENAI_TIMEOUT_SECONDS = 10
ARTICLE_MAX_CHARS = 8000


def strip_html(text: str) -> str:
    return re.sub(r"<[^>]+>", " ", text)


def normalize_text(text: str | None) -> str:
    if not text:
        return ""
    stripped = strip_html(text)
    collapsed = re.sub(r"\s+", " ", stripped)
    return collapsed.strip().lower()


def extract_article(url: str, text_only: bool = False) -> str | None:
    if not url:
        return None

    try:
        downloaded = trafilatura.fetch_url(url)
    except Exception as exc:
        logger.warning(f"Failed to fetch article url for extraction: {exc}")
        return None

    if not downloaded:
        return None

    try:
        if text_only:
            extracted = trafilatura.extract(downloaded, output_format="text")
        else:
            extracted = trafilatura.extract(
                downloaded,
                output_format="html",
                include_comments=True,
                include_tables=True,
                include_images=True,
            )
    except Exception as exc:
        logger.warning(f"Failed to extract article text: {exc}")
        return None

    extracted = extracted or ""
    extracted = extracted.strip()

    if not extracted:
        return None

    return extracted


def summarize_article_with_llm(article_text: str) -> str | None:
    if not _llm_enabled():
        return None

    normalized_text = normalize_text(article_text)
    trimmed_text = _trim_article_text(normalized_text)
    if not trimmed_text:
        return None

    try:
        response_obj = _call_openai_summary_api(trimmed_text)
    except Exception as exc:
        logger.warning(f"LLM article summarization failed: {exc}")
        return None

    response_text = _extract_openai_response_text(response_obj)
    if not response_text:
        return None

    try:
        parsed = json.loads(response_text)
    except json.JSONDecodeError:
        logger.warning("LLM summary response was not valid JSON")
        return None

    summary = parsed.get("summary") or ""
    summary = summary.strip()

    if not summary:
        return None

    return summary + " (AI generated)"


def is_summary_good_with_llm(article_text: str, summary: str) -> bool | None:
    if not _llm_enabled():
        return None

    normalized_text = normalize_text(article_text)
    trimmed_text = _trim_article_text(normalized_text)
    trimmed_summary = (summary or "").strip()
    if not trimmed_text or not trimmed_summary:
        return None

    try:
        response_obj = _call_openai_summary_quality_api(trimmed_text, trimmed_summary)
    except Exception as exc:
        logger.warning(f"LLM summary quality check failed: {exc}")
        return None

    response_text = _extract_openai_response_text(response_obj)
    if not response_text:
        return None

    try:
        parsed = json.loads(response_text)
    except json.JSONDecodeError:
        logger.warning("LLM summary quality response was not valid JSON")
        return None

    if "is_good" not in parsed:
        return None

    return bool(parsed.get("is_good"))


def _llm_enabled() -> bool:
    return Options.get().llm_enabled


def _trim_article_text(content: str) -> str:
    return (content or "")[:ARTICLE_MAX_CHARS]


def _call_openai_summary_api(article_text: str):
    api_key = Options.get().openai_api_key
    if not api_key:
        raise ValueError("OPENAI_API_KEY is not set")

    payload = _build_openai_summary_payload(article_text)
    client = OpenAI(api_key=api_key, timeout=OPENAI_TIMEOUT_SECONDS)
    return client.responses.create(**payload)


def _call_openai_summary_quality_api(article_text: str, summary: str):
    api_key = Options.get().openai_api_key
    if not api_key:
        raise ValueError("OPENAI_API_KEY is not set")

    payload = _build_openai_summary_quality_payload(article_text, summary)
    client = OpenAI(api_key=api_key, timeout=OPENAI_TIMEOUT_SECONDS)
    return client.responses.create(**payload)


def _build_openai_summary_payload(article_text: str) -> dict:
    options = Options.get()
    instructions = (
        "Summarize the article clearly and concisely. Return 3-6 sentences, no bullets, no headings, plain text only."
    )

    user_prompt = "Article:\n" + article_text

    schema = {
        "name": "article_summary",
        "schema": {
            "type": "object",
            "properties": {
                "summary": {"type": "string"},
            },
            "required": ["summary"],
            "additionalProperties": False,
        },
    }

    return {
        "model": options.openai_model,
        "input": [
            {"role": "system", "content": [{"type": "text", "text": instructions}]},
            {"role": "user", "content": [{"type": "text", "text": user_prompt}]},
        ],
        "response_format": {"type": "json_schema", "json_schema": schema},
    }


def _build_openai_summary_quality_payload(article_text: str, summary: str) -> dict:
    options = Options.get()
    instructions = (
        "You are evaluating whether a summary is a good standalone summary of an article. "
        "Answer with is_good=true if it captures the main points and is not just a lead-in."
    )

    user_prompt = "Article:\n" + article_text + "\n\nSummary:\n" + summary

    schema = {
        "name": "summary_quality",
        "schema": {
            "type": "object",
            "properties": {
                "is_good": {"type": "boolean"},
            },
            "required": ["is_good"],
            "additionalProperties": False,
        },
    }

    return {
        "model": options.openai_model,
        "input": [
            {"role": "system", "content": [{"type": "text", "text": instructions}]},
            {"role": "user", "content": [{"type": "text", "text": user_prompt}]},
        ],
        "response_format": {"type": "json_schema", "json_schema": schema},
    }


def _extract_openai_response_text(response) -> str | None:
    if response is None:
        return None

    if not getattr(response, "output", None):
        return None

    for output in response.output:
        for content in getattr(output, "content", []) or []:
            if getattr(content, "type", None) == "output_text":
                text_value = getattr(content, "text", None)
                if text_value:
                    return text_value

    return None
