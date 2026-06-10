-- Add extended metadata fields to media table
ALTER TABLE media ADD COLUMN age_rating TEXT;
ALTER TABLE media ADD COLUMN studio TEXT;
ALTER TABLE media ADD COLUMN trailer_url TEXT;
ALTER TABLE media ADD COLUMN origin_country TEXT;
ALTER TABLE media ADD COLUMN collection_name TEXT;
ALTER TABLE media ADD COLUMN creator TEXT;
ALTER TABLE media ADD COLUMN tags TEXT;
