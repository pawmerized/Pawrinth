CREATE TABLE plus_content_origins (
	instance_id TEXT NOT NULL,
	relative_path TEXT NOT NULL,
	provider TEXT NOT NULL CHECK (provider IN ('curseforge')),
	project_ref TEXT NOT NULL,
	file_ref TEXT NOT NULL,
	PRIMARY KEY (instance_id, relative_path)
);
