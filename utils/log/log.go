package log

import (
	"os"

	"github.com/rs/zerolog"
)

var Logger zerolog.Logger

func init() {
	Logger = zerolog.New(os.Stderr).With().Logger()

	if f := os.Getenv("BUSHI_DEBUG"); f != "" {
		zerolog.SetGlobalLevel(zerolog.DebugLevel)
	} else {
		zerolog.SetGlobalLevel(zerolog.InfoLevel)
	}
}
