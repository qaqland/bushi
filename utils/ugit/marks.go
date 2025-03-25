package ugit

import (
	"os"
)

func InitFile(path string, clear bool) error {
	_, err := os.Stat(path)
	if os.IsNotExist(err) {
		file, err := os.Create(path)
		if err != nil {
			return err
		}
		defer file.Close()
	} else if err == nil && clear {
		file, err := os.OpenFile(path, os.O_TRUNC|os.O_WRONLY, 0644)
		if err != nil {
			return err
		}
		defer file.Close()
	} else if err != nil {
		return err
	}
	return nil
}
