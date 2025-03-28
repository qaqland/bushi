package main

import (
	"fmt"
	"os"
	"runtime/pprof"

	"bushi/config"
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
	if err := server.Initialize(&cfg); err != nil {
	}

}
