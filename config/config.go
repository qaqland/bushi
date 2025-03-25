package config

import (
	"bushi/utils/log"
	"bushi/utils/ugit"

	"path/filepath"

	"github.com/BurntSushi/toml"
)

var logger = log.Logger.With().Str("service", "config").Logger()

type Config struct {
	Www      Www      `json:"www"`
	Database Database `json:"database"`
	Repo     []Repo   `json:"repositories"`
}

type Www struct {
	Name string `json:"name"`
	Desc string `json:"description"`
}

type Database struct {
	Sqlite  string `json:"sqlite"`
	MarkDir string `json:"mark_dir" toml:"mark_dir"`
}

type Repo struct {
	Name string `json:"name"`
	Desc string `json:"description"`
	Path string `json:"path"`
	Head string `json:"head"`
}

func NewConfig(path string) Config {
	logger.Info().Str("path", path).Msg("load")

	var config Config
	meta, err := toml.DecodeFile(path, &config)
	if err != nil {
		logger.Fatal().Err(err).Send()
	}

	// TODO more default settings
	if !meta.IsDefined("www", "name") {
		config.Www.Name = "bushi example"
	}

	if err := ugit.CheckGitExists(); err != nil {
		logger.Fatal().Err(err).Send()
	}

	// mark_dir must be absolute
	config.Database.MarkDir, _ = filepath.Abs(config.Database.MarkDir)

	for i := range config.Repo {
		// set default repository name
		if config.Repo[i].Name == "" {
			config.Repo[i].Name = ugit.NameFromPath(config.Repo[i].Path)
		}
		// check if path is directly point to git_dir
		if !ugit.IsGitDir(config.Repo[i].Path) {
			logger.Fatal().Str("path", config.Repo[i].Path).Msg("not git_dir")
		}
		// init mark files
		marks := filepath.Join(config.Database.MarkDir, config.Repo[i].Name)
		if err := ugit.InitFile(marks, false); err != nil {
			logger.Fatal().Err(err).Str("marks", marks).Msg("init marks")
		}
	}

	logger.Info().Interface("content", config).Send()

	return config
}
