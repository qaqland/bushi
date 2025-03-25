package config

import (
	"os"
	"testing"
)

func tmpConfig(t *testing.T, content string) string {
	t.Helper()
	tmpfile, err := os.CreateTemp("", "config*.toml")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { os.Remove(tmpfile.Name()) })

	if _, err := tmpfile.WriteString(content); err != nil {
		t.Fatal(err)
	}
	if err := tmpfile.Close(); err != nil {
		t.Fatal(err)
	}
	return tmpfile.Name()
}

func TestGetConfig(t *testing.T) {
	t.Run("(empty)", func(t *testing.T) {
		path := tmpConfig(t, ``)
		config := NewConfig(path)
		if config.Www.Name != "bushi example" {
			t.Errorf("get %q, want %q", config.Www.Name, "bushi example")
		}
	})

	t.Run("(value)", func(t *testing.T) {
		path := tmpConfig(t, `
[www]
name = "mygit"
`)
		config := NewConfig(path)
		if config.Www.Name != "mygit" {
			t.Errorf("get %q, want %q", config.Www.Name, "mygit")
		}
	})
}
