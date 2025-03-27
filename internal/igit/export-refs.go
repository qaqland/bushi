package igit

import (
	"io"
	"strings"

	"bushi/database"

	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	"github.com/go-git/go-git/v5/plumbing/object"
)

// refs are full names
func ForEachRef(repo *database.Repository, refs ...string) func() (*database.Reference, bool) {
	length := len(refs)
	r, _ := git.PlainOpen(repo.Path)

	if length == 0 {
		iter, err := r.References()
		if err != nil {
			logger.Panic().Err(err).Msg("failed to call repo.References")
		}
		return func() (*database.Reference, bool) {
			gref, err := iter.Next()
			if err == io.EOF {
				iter.Close()
				logger.Info().Msg("Done")
				return nil, false
			}
			if err != nil {
				return nil, true
			}
			ref, err := get_one(r, gref)
			if err != nil {
				return nil, true
			}
			if ref != nil {
				ref.RepositoryID = repo.ID
			}
			return ref, true
		}
	}

	index := 0
	return func() (*database.Reference, bool) {
		if index >= length {
			logger.Info().Msg("Done")
			return nil, false
		}
		defer func() {
			index++
		}()
		name := plumbing.ReferenceName(refs[index])
		gref, err := r.Reference(name, true)
		if err != nil {
			return nil, true
		}
		ref, err := get_one(r, gref)
		if err != nil {
			return nil, true
		}
		if ref != nil {
			ref.RepositoryID = repo.ID
		}
		return ref, true
	}
}

func get_one(repo *git.Repository, gref *plumbing.Reference) (*database.Reference, error) {
	name := gref.Name()
	hash := gref.Hash()

	if !name.IsBranch() && !name.IsTag() {
		return nil, nil
	}

	if err := name.Validate(); err != nil {
		return nil, err
	}

	rref := database.Reference{
		FullName:  name,
		ShortName: strings.ReplaceAll(name.Short(), "/", ":"),
		IsTag:     name.IsTag(),
	}

	obj, err := repo.Object(plumbing.AnyObject, hash)
	if err != nil {
		return nil, err
	}
	switch o := obj.(type) {
	case *object.Commit:
		rref.CommitObj = o
	case *object.Tag:
		cobj, err := o.Commit()
		if err == object.ErrUnsupportedObject {
			logger.Info().
				Str("Tag", gref.String()).
				Msg("tag on non-commit object is not supported")
			return nil, nil
		}
		if err != nil {
			return nil, err
		}
		rref.CommitObj = cobj
	}

	// TODO copy time from commit to ref
	rref.Time = rref.CommitObj.Committer.When.Unix()

	return &rref, nil
}
