CREATE TABLE plus_curseforge_packs (
	instance_id TEXT PRIMARY KEY NOT NULL,
	project_id INTEGER NOT NULL,
	file_id INTEGER NOT NULL,
	name TEXT NOT NULL,
	version TEXT NOT NULL,
	missing TEXT NOT NULL DEFAULT '[]',
	kept TEXT NOT NULL DEFAULT '[]',
	overrides TEXT NOT NULL DEFAULT '{}'
);
