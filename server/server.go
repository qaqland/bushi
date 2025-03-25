package server

import (
	"sync"
)

type Server struct {
	Repositories sync.Map
}
