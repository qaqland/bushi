package database

import (
	"context"
	"fmt"
	"time"

	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/clause"
	gorm_log "gorm.io/gorm/logger"

	"bushi/utils/log"
)

const sqliteTimeout = 5 * time.Second

var logger = log.Logger.With().Str("service", "sqlite").Logger()

type SqliteDB struct {
	*gorm.DB
}

func NewSqliteDB(path string) Database {
	gorm_config := gorm.Config{
		CreateBatchSize: 64,
		PrepareStmt:     true,
		Logger:          gorm_log.Discard,
		// Logger:          gorm_log.Default.LogMode(gorm_log.Info),
	}

	dsn := fmt.Sprintf("%s?journal=MEMORY&_sync=OFF", path)
	db, err := gorm.Open(sqlite.Open(dsn), &gorm_config)
	if err != nil {
		logger.Fatal().Err(err).Msg("failed to open")
	}
	if err := AutoMigrate(db); err != nil {
		logger.Fatal().Err(err).Msg("failed to automigrate")
	}
	return &SqliteDB{db}
}

func (db *SqliteDB) StoreCommit(ctx context.Context, commit *Commit) error {
	ctx, cancel := context.WithTimeout(ctx, sqliteTimeout)
	defer cancel()

	result := db.WithContext(ctx).Create(commit)
	return result.Error
}

func (db *SqliteDB) StoreRepository(ctx context.Context, repo *Repository) error {
	ctx, cancel := context.WithTimeout(ctx, sqliteTimeout)
	defer cancel()

	// RepositoryID should be 0, use Name to find
	result := db.WithContext(ctx).Where(repo).FirstOrCreate(repo)
	return result.Error
}

func (db *SqliteDB) StoreReference(ctx context.Context, ref *Reference) error {
	ctx, cancel := context.WithTimeout(ctx, sqliteTimeout)
	defer cancel()

	result := db.WithContext(ctx).Clauses(
		clause.OnConflict{
			Columns:   []clause.Column{{Name: "full_name"}, {Name: "repository_id"}},
			UpdateAll: true,
		}).Create(ref)
	return result.Error
}
