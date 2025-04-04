SELECT DISTINCT commits.*
FROM commits
	INNER JOIN commit_files ON commits.commit_id = commit_files.commit_id
	INNER JOIN files ON commit_files.file_id = files.file_id
WHERE files.name LIKE 'community/xmake%';
