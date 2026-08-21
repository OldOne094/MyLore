-- MISSION-083: reading recap aggregates.
-- The reading recap scans consumed nodes by their completion timestamp
-- (read_at) across a year window, so an index on read_at keeps the range scan
-- cheap as node_progress grows.
CREATE INDEX idx_node_progress_read_at ON node_progress(read_at);