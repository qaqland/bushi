package controller

import (
	"net/http"

	"github.com/gin-gonic/gin"
)

func todo(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{
		"message": "todo",
	})
}

func SetupRoutes(r *gin.RouterGroup) {
	r.GET("/:repo/-/about", todo)
	r.GET("/:repo/-/summary", todo)
	r.GET("/:repo/-/tags", todo)
	r.GET("/:repo/-/heads", todo)

	// commit
	r.GET("/:repo/-/commit/:oid", todo)
	r.GET("/:repo/-/patch/:oid", todo)

	// action: tree, blob, log, raw
	r.GET("/:repo/-/:action/:commit", todo)
	r.GET("/:repo/-/:action/:commit/*filepath", todo)
	r.GET("/:repo/-/:action/head/:branch", todo)
	r.GET("/:repo/-/:action/head/:branch/*filepath", todo)
	r.GET("/:repo/-/:action/tag/:tag", todo)
	r.GET("/:repo/-/:action/tag/:tag/*filepath", todo)

	// local hook
	r.GET("/hook/:key", todo)
}
