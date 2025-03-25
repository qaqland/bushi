package main

import (
	"flag"
	"fmt"
	"log"
	"os"
	"strings"
)

type Event struct {
	OldOid  string
	NewOid  string
	RefName string
}

func ParseEvent(line string) Event {
	parts := strings.Fields(line)
	if len(parts) < 3 {
		log.Fatalln("Invalid input format", line)
	}
	return Event{
		OldOid:  parts[0],
		NewOid:  parts[1],
		RefName: parts[2],
	}
}

func main() {
	url_path := flag.String("u", "/run/bushi/post-receive-hook", "file contains url")
	flag.Parse()
	fmt.Println(*url_path)
	git_dir, has_env := os.LookupEnv("GIT_DIR")
	if has_env {
		fmt.Println(git_dir)
	}
	// TODO
	// get url from path
	// parse message from stdin
	// send post request
}
