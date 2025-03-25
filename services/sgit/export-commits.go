package sgit

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"

	"bushi/database"
	"bushi/utils/log"
)

var logger = log.Logger.With().Str("service", "internal").Logger()

type state struct {
	data_skip int
	is_commit bool
	mark      uint
	from      uint
	oid       string
	diff      []string
}

func parse_mark(value string) (uint, error) {
	mark := strings.TrimLeft(value, ":")
	res, err := strconv.ParseUint(mark, 10, 0)
	if err != nil {
		return 0, err
	}
	return uint(res), nil
}

func parse_diff(value string) (string, error) {
	parts := strings.SplitN(value, " ", 3)
	if len(parts) < 3 {
		return "", fmt.Errorf("failed to parse diff: %s", value)
	}
	return parts[2], nil
}

func (s *state) parse(reader *bufio.Reader) (*database.Commit, error) {
	r, err := reader.Discard(s.data_skip)
	if err != nil {
		return nil, err
	}
	s.data_skip -= r

	line, err := reader.ReadString('\n')
	if err != nil {
		return nil, err
	}
	line = strings.TrimSpace(line)

	// commit is done
	if len(line) == 0 {
		defer func() {
			*s = state{}
		}()
		if s.is_commit {
			files := make([]database.File, len(s.diff))
			for i, s := range s.diff {
				files[i] = database.File{
					Name: s,
				}
			}
			return &database.Commit{
				Oid:        s.oid,
				Mark:       s.mark,
				ParentMark: s.from,
				Files:      files,
			}, nil
		}
		return nil, nil
	}

	tokens := strings.SplitN(line, " ", 2)
	// TODO check length
	key, value := tokens[0], tokens[1]

	var diff_file string

	switch key {
	case "commit":
		s.is_commit = true
	case "mark":
		s.mark, err = parse_mark(value)
	case "original-oid":
		s.oid = value
	case "data":
		s.data_skip, err = strconv.Atoi(value)
	case "from":
		s.from, err = parse_mark(value)
	case "M":
		diff_file, err = parse_diff(value)
		s.diff = append(s.diff, diff_file)
	default:
		// skip others
	}
	if err != nil {
		return nil, err
	}
	return nil, err
}

func FastExport(repo *database.Repository, mark_dir string) func() (*database.Commit, bool) {
	marks := filepath.Join(mark_dir, repo.Name)
	logger.Info().
		Str("git_dir", repo.Path).
		Str("marks", marks).
		Str("command", "git fast-export").
		Send()
	gitcmd := exec.Command("git",
		"fast-export",
		"--signed-tags=strip",
		"--export-marks",
		marks,
		"--import-marks",
		marks,
		"--mark-tags",
		"--fake-missing-tagger",
		"--no-data",
		"--show-original-ids",
		"--reencode=yes",
		"--branches",
		"--tags")
	gitcmd.Dir = repo.Path
	gitcmd.Stderr = os.Stderr
	stdout, err := gitcmd.StdoutPipe()
	if err != nil {
		logger.Fatal().Err(err).Msg("stdout pipe")
	}
	if err := gitcmd.Start(); err != nil {
		logger.Fatal().Err(err).Msg("start")
	}

	outputChan := make(chan *database.Commit)
	var wg sync.WaitGroup
	wg.Add(1)

	go func() {
		defer wg.Done()
		reader := bufio.NewReader(stdout)
		var state state
		for {
			commit, err := state.parse(reader)
			if err != nil {
				if err == io.EOF {
					logger.Info().Msg("done")
					break
				} else {
					logger.Fatal().Err(err).Msg("parse")
				}
			}
			if commit != nil {
				commit.RepositoryID = repo.ID
				outputChan <- commit
			}
		}
		close(outputChan)
		if err := gitcmd.Wait(); err != nil {
			logger.Fatal().Err(err).Msg("wait")
		}
	}()

	return func() (*database.Commit, bool) {
		c, ok := <-outputChan
		return c, ok
	}
}
