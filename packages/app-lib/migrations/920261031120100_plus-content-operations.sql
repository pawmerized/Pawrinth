CREATE TABLE plus_content_operations (
	id TEXT PRIMARY KEY NOT NULL,
	instance_id TEXT NOT NULL,
	kind TEXT NOT NULL CHECK (kind IN ('update_all', 'update_one', 'rollback', 'snapshot')),
	state TEXT NOT NULL CHECK (state IN ('applying', 'applied', 'partial', 'reverted', 'captured', 'needs_attention')),
	parent_id TEXT NULL,
	label TEXT NULL,
	game_version TEXT NOT NULL,
	loader TEXT NOT NULL,
	loader_version TEXT NULL,
	manifest TEXT NOT NULL,
	scope TEXT NULL,
	last_error TEXT NULL,
	created_at INTEGER NOT NULL,
	finished_at INTEGER NULL
);

CREATE INDEX plus_content_operations_instance
ON plus_content_operations (instance_id, created_at);

CREATE TABLE plus_content_operation_items (
	operation_id TEXT NOT NULL REFERENCES plus_content_operations (id) ON DELETE CASCADE,
	ordinal INTEGER NOT NULL,
	change TEXT NOT NULL CHECK (change IN ('updated', 'added', 'removed', 'toggled')),
	before_entry TEXT NULL,
	after_entry TEXT NULL,
	PRIMARY KEY (operation_id, ordinal)
);
