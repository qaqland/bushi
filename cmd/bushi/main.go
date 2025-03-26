package main

import (
	"context"
	"fmt"
	"os"
	"runtime/pprof"

	"bushi/config"
	"bushi/database"
	"bushi/internal/igit"
	"bushi/server"
	"bushi/utils/log"
)

var logger = log.Logger.With().Str("service", "main").Logger()
var config_path = "tests/example.toml"

func main() {
	fmt.Println(">>> bushi")

	if cpuprofile := os.Getenv("BUSHI_CPU"); cpuprofile != "" {
		f, _ := os.Create(cpuprofile)
		pprof.StartCPUProfile(f)
		defer pprof.StopCPUProfile()
	}

	if path := os.Getenv("BUSHI_CONFIG"); path != "" {
		config_path = path
	}
	cfg := config.NewConfig(config_path)

	db := database.NewSqliteDB(cfg)
	server := server.Server{}

	for _, e := range cfg.Repo {
		repo := database.Repository{
			Name: e.Name,
			Head: e.Head,
			Desc: e.Desc,
			Path: e.Path,
		}
		if err := db.StoreRepository(context.TODO(), &repo); err != nil {
			logger.Fatal().Err(err).Str("path", repo.Path).Msg("store repository")
		}

		server.Repositories.Store(repo.Name, &repo)

		// continue

		iter := igit.FastExport(&repo, cfg.Database.MarkDir)
		for {
			commit, ok := iter()
			if !ok {
				break
			}
			commit.RepositoryID = repo.ID
			if err := db.StoreCommit(context.TODO(), commit); err != nil {
				logger.Fatal().Err(err).Str("oid", commit.Oid).Msg("store commit")
			}
		}
	}
}
