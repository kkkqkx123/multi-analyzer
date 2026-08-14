package main

import (
	"testing"

	"go-demo/internal/utils"
)

func TestMainFlow(t *testing.T) {
	if utils.Add(2, 3) != 5 {
		t.Fatal("Add(2,3) should be 5")
	}
}
