package sgit

import (
	"io"

	"bushi/database"

	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	"github.com/go-git/go-git/v5/plumbing/object"
)

// refs is full names
func SyncRefs(repo *database.Repository, refs ...string) func() (*database.Reference, error) {
	length := len(refs)
	r, _ := git.PlainOpen(repo.Path)

	if length == 0 {

		iter, err := r.References()
		return func() (*database.Reference, error) {
			if err != nil {
				iter.Close() // Necessary?
				return nil, err
			}
			gref, err := iter.Next()
			if err != nil {
				return nil, err
			}

			ref, err := get_one(r, gref)
			if err != nil {
				return nil, err
			}
			if ref != nil {
				ref.RepositoryID = repo.ID
			}
			return ref, nil
		}
	}

	index := 0
	return func() (*database.Reference, error) {
		if index >= length {
			return nil, io.EOF
		}
		defer func() {
			index++
		}()
		name := plumbing.ReferenceName(refs[index])
		gref, err := r.Reference(name, true)
		if err != nil {
			return nil, err
		}
		ref, err := get_one(r, gref)
		// if err == object.ErrUnsupportedObject {
		// skip tag on tree or blob TODO log this warning, put it outside
		// }
		if err != nil {
			return nil, err
		}
		if ref != nil {
			ref.RepositoryID = repo.ID
		}
		return ref, nil
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
		FullName: name,
	}

	obj, err := repo.Object(plumbing.AnyObject, hash)
	if err != nil {
		return nil, err
	}
	switch o := obj.(type) {
	case *object.Commit:
		rref.Type = database.TagLw
		rref.CommitObj = o
	case *object.Tag:
		rref.Type = database.TagAn
		cobj, err := o.Commit()
		if err != nil {
			// tag on tree or blob
			return nil, err
		}
		rref.CommitObj = cobj
	}

	if name.IsBranch() {
		rref.Type = database.Branch
	}

	// TODO copy commit time to ref

	return &rref, nil
}
