package ugit

import (
	"errors"
	"os/exec"
	"path/filepath"
	"strings"
)

func NameFromPath(path string) string {
	cleaned := filepath.Clean(path)
	base := filepath.Base(cleaned)

	if base == ".git" {
		parent := filepath.Dir(cleaned)
		return filepath.Base(parent)
	}

	return strings.TrimSuffix(base, ".git")
}

func CheckGitExists() error {
	cmd := exec.Command("git", "version")
	if err := cmd.Run(); err != nil {
		return errors.New("git executable not found")
	}
	return nil
}

func IsGitDir(git_dir string) bool {
	cmd := exec.Command("git", "rev-parse", "--is-inside-git-dir")
	cmd.Dir = git_dir
	out, err := cmd.CombinedOutput()
	if err != nil {
		return false
	}
	return strings.TrimSpace(string(out)) == "true"
}
