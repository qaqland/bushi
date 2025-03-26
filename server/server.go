package server

import (
	"sync"

	"bushi/config"

	"gorm.io/gorm"
)

type Server struct {
	Repositories sync.Map
	database     *gorm.DB
}

var instance *Server

func Initialize(config *config.Config) error {
	instance = &Server{}
	return nil
}

func GetInstance() *Server {
	return instance
}
