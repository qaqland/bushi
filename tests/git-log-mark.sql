SELECT DISTINCT commits.*
FROM commits
JOIN commit_files ON commits.commit_id = commit_files.commit_id
JOIN files ON commit_files.file_id = files.file_id
WHERE files.name LIKE 'community/xmake%';

