package main

import (
	"fmt"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/spf13/cobra"
)

func healthHandler(w http.ResponseWriter, r *http.Request) {
	w.WriteHeader(http.StatusOK)
	fmt.Fprint(w, "ok")
}

func getUsersHandler(w http.ResponseWriter, r *http.Request) {
	w.WriteHeader(http.StatusOK)
	fmt.Fprint(w, "[]")
}

func createUserHandler(w http.ResponseWriter, r *http.Request) {
	w.WriteHeader(http.StatusCreated)
	fmt.Fprint(w, "{}")
}

func loggingMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		fmt.Println(r.Method, r.URL.Path)
		next.ServeHTTP(w, r)
	})
}

func main() {
	// net/http route entrypoint
	http.HandleFunc("/health", healthHandler)

	// chi router entrypoints
	r := chi.NewRouter()
	r.Use(loggingMiddleware)
	r.Get("/api/users", getUsersHandler)
	r.Post("/api/users", createUserHandler)

	// cobra CLI entrypoint
	rootCmd := &cobra.Command{
		Use:   "server",
		Short: "Framework entrypoints fixture server",
		Run: func(cmd *cobra.Command, args []string) {
			http.ListenAndServe(":8080", r)
		},
	}
	rootCmd.Execute()
}
