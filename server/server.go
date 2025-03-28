package server

import (
	"context"
	"sync"

	"bushi/config"
	"bushi/database"
	"bushi/internal/igit"
	"bushi/utils/log"
)

type Server struct {
	Repositories sync.Map
	Database     database.Database
	MarkDir      string
}

var instance *Server
var logger = log.Logger.With().Str("service", "server").Logger()

func Initialize(config *config.Config) error {
	instance = &Server{
		MarkDir:  config.Database.MarkDir,
		Database: database.NewSqliteDB(config.Database.Sqlite),
	}

	for _, r := range config.Repo {
		repo := database.Repository{
			Name: r.Name,
			Head: r.Head,
			Desc: r.Desc,
			Path: r.Path,
		}
		instance.Repositories.Store(r.Name, &repo)

		if err := instance.Database.StoreRepository(context.TODO(), &repo); err != nil {
			logger.Fatal().Err(err).Str("path", repo.Path).Msg("store repository")
		}

		commit_iter := igit.FastExport(&repo, instance.MarkDir)
		for {
			commit, ok := commit_iter()
			if !ok {
				break
			}
			if err := instance.Database.StoreCommit(context.TODO(), commit); err != nil {
				logger.Fatal().Err(err).Str("oid", commit.Oid).Msg("store commit")
			}
		}

		ref_iter := igit.ForEachRef(&repo)
		for {
			ref, ok := ref_iter()
			if !ok {
				break
			}
			if ref == nil {
				continue
			}
			if err := instance.Database.StoreReference(context.TODO(), ref); err != nil {
				logger.Panic().
					Err(err).
					Str("oid", ref.FullName.String()).
					Msg("store reference")
			}
		}
	}
	return nil
}

func GetInstance() *Server {
	return instance
}
