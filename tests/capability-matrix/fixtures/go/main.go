package main

import (
	"log"

	"example.com/capability-matrix/util"
)

type Widget struct {
	Name string
}

func Build(input string) string {
	local := input
	if local == "" {
		return "empty"
	}
	return local
}

func (w Widget) Label() string {
	return w.Name
}

func handler(token string) {
	dangerous()
	writeBalance()
	tx := Begin()
	_ = tx
	log.Println(token)
	_ = util.Helper("matrix-literal")
}

func main() {
	w := Widget{Name: "ok"}
	local := Build(w.Label())
	handler(local)
}

func dangerous() {}
func writeBalance() {}
func authorize() {}
func Begin() *Tx { return &Tx{} }
func Rollback() {}

type Tx struct{}
