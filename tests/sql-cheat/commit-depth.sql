CREATE TRIGGER IF NOT EXISTS auto_depth
AFTER INSERT ON commits
FOR EACH ROW
WHEN NEW.parent_id IS NOT NULL
BEGIN
	UPDATE commits
	SET depth = (
		SELECT depth + 1
		FROM commits
		WHERE commit_id = NEW.parent_id
	)
	WHERE commit_id = NEW.commit_id;
END;
