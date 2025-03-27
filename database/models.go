package database

import (
	"context"
	"errors"
	"fmt"

	"github.com/go-git/go-git/v5/plumbing"
	"github.com/go-git/go-git/v5/plumbing/object"
	"gorm.io/gorm"
)

type Database interface {
	StoreCommit(ctx context.Context, commit *Commit) error

	StoreRepository(ctx context.Context, repo *Repository) error
}

type Repository struct {
	ID   uint   `gorm:"primarykey"`
	Name string `gorm:"uniqueIndex" json:"name"`
	Head string `gorm:"-"`
	Desc string `gorm:"-"`
	Path string `gorm:"-"`
}

type Commit struct {
	ID           uint   `gorm:"primarykey"`
	Oid          string `gorm:"uniqueIndex:idx_c_oid"`
	Mark         uint   `gorm:"uniqueIndex:idx_c_mark"`
	RepositoryID uint   `gorm:"uniqueIndex:idx_c_oid;uniqueIndex:idx_c_mark"`
	Repository   Repository
	ParentID     *uint
	Parent       *Commit
	ParentMark   uint   `gorm:"-"`
	Files        []File `gorm:"many2many:commit_files;"`
}

func (c *Commit) BeforeCreate(tx *gorm.DB) error {
	if c.RepositoryID == 0 {
		return errors.New("Commit must specify its RepositoryID")
	}

	if c.ParentMark != 0 && c.ParentID == nil {
		var parent Commit
		err := tx.
			Select("id").
			Where("repository_id = ? AND mark = ?", c.RepositoryID, c.ParentMark).
			First(&parent).
			Error
		if err != nil {
			return fmt.Errorf("Failed to find mark %d in repository %d", c.ParentMark, c.RepositoryID)
		}
		c.ParentID = &parent.ID
	}

	for i := range c.Files {
		err := tx.
			Where(File{Name: c.Files[i].Name}).
			FirstOrCreate(&c.Files[i]).
			Error
		if err != nil {
			return err
		}
	}

	return nil
}

func (c *Commit) String() string {
	return fmt.Sprintf("Commit %d %s %d: %d", c.ID, c.Oid, c.Mark, len(c.Files))
}

type File struct {
	ID   uint   `gorm:"primarykey"`
	Name string `gorm:"unique"`
}

type Reference struct {
	ID           uint                   `gorm:"primarykey"`
	ShortName    string                 `gorm:"index"`
	FullName     plumbing.ReferenceName `gorm:"index"`
	Time         int64                  `gorm:"index"`
	IsTag        bool                   // drop?
	CommitID     uint
	CommitObj    *object.Commit `gorm:"-"`
	Commit       Commit
	RepositoryID uint
	Repository   Repository
}

func (r *Reference) BeforeSave(tx *gorm.DB) error {
	if r.CommitObj == nil {
		return errors.New("Reference must have a commit target")
	}
	var cobj Commit
	err := tx.
		Select("id").
		Where("repository_id = ? AND oid = ?", r.RepositoryID, r.CommitObj.Hash).
		First(&cobj).
		Error
	if err != nil {
		return fmt.Errorf("Failed to find commit %d in repository %d", r.CommitObj.Hash, r.RepositoryID)
	}
	r.CommitID = cobj.ID
	return nil
}

func AutoMigrate(db *gorm.DB) error {
	return db.AutoMigrate(
		&Commit{},
		&Repository{},
		&File{},
		&Repository{},
	)
}
