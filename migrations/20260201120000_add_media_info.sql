-- Add media_info column to store detailed ffprobe data JSON
ALTER TABLE media ADD COLUMN media_info TEXT;
