ALTER TABLE feed ADD COLUMN manual_use_extracted_fulltext BOOLEAN;
ALTER TABLE feed ADD COLUMN manual_use_llm_summary BOOLEAN;
ALTER TABLE feed ADD COLUMN last_manual_quality_override INTEGER;
