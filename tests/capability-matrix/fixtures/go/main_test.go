package main

import "testing"

func TestBuild(t *testing.T) {
	if Build("x") != "x" {
		t.Fatal("unexpected")
	}
}
