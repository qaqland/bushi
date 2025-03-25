package ugit

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/go-git/go-git/v5"
)

func TestNameFromPath(t *testing.T) {
	tests := []struct {
		path string
		name string
	}{
		// basic
		{"aports/.git", "aports"},
		{"aports.git", "aports"},
		{"aports", "aports"},

		// more
		{"/tmp/repo/myrepo.git", "myrepo"},
	}
	for i, tt := range tests {
		tn := fmt.Sprintf("%d-%s", i, tt.path)
		t.Run(tn, func(t *testing.T) {
			name := NameFromPath(tt.path)
			if name != tt.name {
				t.Errorf("(%q) -> %q, want %q", tt.path, name, tt.name)
			}
		})
	}
}

func TestIsGitDir(t *testing.T) {

	t.Run("bare", func(t *testing.T) {
		dir, _ := os.MkdirTemp("", "repo-*")
		defer os.RemoveAll(dir)

		git.PlainInit(dir, true)
		if !IsGitDir(dir) {
			t.Errorf("bare is git_dir")
		}
	})

	t.Run("normal-dotgit", func(t *testing.T) {
		dir, _ := os.MkdirTemp("", "repo-*")
		defer os.RemoveAll(dir)

		git.PlainInit(dir, false)
		if !IsGitDir(filepath.Join(dir, ".git")) {
			t.Errorf("normal dotgit is git_dir")
		}
	})

	t.Run("normal-work", func(t *testing.T) {
		dir, _ := os.MkdirTemp("", "repo-*")
		defer os.RemoveAll(dir)

		git.PlainInit(dir, false)
		if IsGitDir(dir) {
			t.Errorf("worktree is not git_dir")
		}
	})

}
