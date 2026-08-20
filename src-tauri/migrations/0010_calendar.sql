-- MISSION-081: calendar range queries.
-- The month grid buckets content-node air/release dates and the user activity
-- trail by local day; both reads scan a whole month, so they need plain date
-- indexes (activity.created_at is RFC3339; content_node.release_date is ISO).

CREATE INDEX idx_activity_created_at ON activity(created_at);
CREATE INDEX idx_content_node_release_date ON content_node(release_date);